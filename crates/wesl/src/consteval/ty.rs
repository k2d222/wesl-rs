use super::{
    ArrayInstance, ConstEvalError, Context, Eval, Instance, LiteralInstance, MatInner, MatInstance,
    PtrInstance, RefInstance, StructInstance, SyntaxUtil, VecInner, VecInstance,
};

use wgsl_parse::syntax::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Bool,
    AbstractInt,
    AbstractFloat,
    I32,
    U32,
    F32,
    F16,
    Struct(String),
    Array(Option<usize>, Box<Type>),
    Vec(u8, Box<Type>),
    Mat(u8, u8, Box<Type>),
    Atomic(Box<Type>),
    Ptr(Box<Type>),
    Void,
}

impl Type {
    /// reference: https://www.w3.org/TR/WGSL/#scalar
    pub fn is_scalar(&self) -> bool {
        match self {
            Type::Bool
            | Type::AbstractInt
            | Type::AbstractFloat
            | Type::I32
            | Type::U32
            | Type::F32
            | Type::F16 => true,
            _ => false,
        }
    }

    /// reference: https://www.w3.org/TR/WGSL/#numeric-scalar
    pub fn is_numeric(&self) -> bool {
        match self {
            Type::AbstractInt
            | Type::AbstractFloat
            | Type::I32
            | Type::U32
            | Type::F32
            | Type::F16 => true,
            _ => false,
        }
    }

    /// reference: https://www.w3.org/TR/WGSL/#integer-scalar
    pub fn is_integer(&self) -> bool {
        match self {
            Type::AbstractInt | Type::I32 | Type::U32 => true,
            _ => false,
        }
    }

    /// reference: https://www.w3.org/TR/WGSL/#abstract-types
    pub fn is_abstract(&self) -> bool {
        match self {
            Type::AbstractInt => true,
            Type::AbstractFloat => true,
            Type::Array(ty) | Type::Vec(_, ty) | Type::Mat(_, _, ty) => ty.is_abstract(),
            _ => false,
        }
    }

    pub fn is_concrete(&self) -> bool {
        !self.is_abstract()
    }
}

pub trait Ty {
    /// get the type of an instance.
    fn ty(&self) -> Type;

    /// get the inner type of an instance (not recursive).
    ///
    /// e.g. the inner type of `array<vec3<u32>>` is `vec3<u32>`.
    fn inner_ty(&self) -> Type {
        self.ty()
    }
}

impl Ty for Type {
    fn ty(&self) -> Type {
        self.clone()
    }

    fn inner_ty(&self) -> Type {
        match self {
            Type::Bool => self.clone(),
            Type::AbstractInt => self.clone(),
            Type::AbstractFloat => self.clone(),
            Type::I32 => self.clone(),
            Type::U32 => self.clone(),
            Type::F32 => self.clone(),
            Type::F16 => self.clone(),
            Type::Struct(_) => self.clone(),
            Type::Array(ty) => ty.inner_ty(),
            Type::Vec(_, ty) => ty.inner_ty(),
            Type::Mat(_, _, ty) => ty.inner_ty(),
            Type::Atomic(ty) => ty.inner_ty(),
            Type::Ptr(_) => self.clone(),
            Type::Void => self.clone(),
        }
    }
}

impl Ty for Instance {
    fn ty(&self) -> Type {
        match self {
            Instance::Literal(l) => l.ty(),
            Instance::Struct(s) => s.ty(),
            Instance::Array(a) => a.ty(),
            Instance::Vec(v) => v.ty(),
            Instance::Mat(m) => m.ty(),
            Instance::Ptr(p) => p.ty(),
            Instance::Ref(r) => r.ty(),
            Instance::Type(t) => t.clone(),
            Instance::Void => Type::Void,
        }
    }
}

impl Ty for LiteralInstance {
    fn ty(&self) -> Type {
        match self {
            LiteralInstance::Bool(_) => Type::Bool,
            LiteralInstance::AbstractInt(_) => Type::AbstractInt,
            LiteralInstance::AbstractFloat(_) => Type::AbstractFloat,
            LiteralInstance::I32(_) => Type::I32,
            LiteralInstance::U32(_) => Type::U32,
            LiteralInstance::F32(_) => Type::F32,
            LiteralInstance::F16(_) => Type::F16,
        }
    }
}
impl Ty for StructInstance {
    fn ty(&self) -> Type {
        Type::Struct(self.name().to_string())
    }
}

impl Ty for ArrayInstance {
    fn ty(&self) -> Type {
        Type::Array(Box::new(self.inner_ty().clone()))
    }
    fn inner_ty(&self) -> Type {
        self.components[0].ty()
    }
}

impl<const N: usize> Ty for VecInner<N> {
    fn ty(&self) -> Type {
        Type::Vec(N as u8, Box::new(self.inner_ty()))
    }
    fn inner_ty(&self) -> Type {
        self.components[0].ty()
    }
}

impl Ty for VecInstance {
    fn ty(&self) -> Type {
        Type::Vec(self.n(), Box::new(self.inner_ty()))
    }
    fn inner_ty(&self) -> Type {
        match self {
            VecInstance::Vec2(v) => v.inner_ty(),
            VecInstance::Vec3(v) => v.inner_ty(),
            VecInstance::Vec4(v) => v.inner_ty(),
        }
    }
}

impl<const C: usize, const R: usize> Ty for MatInner<C, R> {
    fn ty(&self) -> Type {
        Type::Mat(C as u8, R as u8, Box::new(self.inner_ty()))
    }
    fn inner_ty(&self) -> Type {
        self.components[0].ty()
    }
}

impl Ty for MatInstance {
    fn ty(&self) -> Type {
        Type::Mat(self.c(), self.r(), Box::new(self.inner_ty()))
    }
    fn inner_ty(&self) -> Type {
        match self {
            MatInstance::Mat2x2(m) => m.inner_ty(),
            MatInstance::Mat2x3(m) => m.inner_ty(),
            MatInstance::Mat2x4(m) => m.inner_ty(),
            MatInstance::Mat3x2(m) => m.inner_ty(),
            MatInstance::Mat3x3(m) => m.inner_ty(),
            MatInstance::Mat3x4(m) => m.inner_ty(),
            MatInstance::Mat4x2(m) => m.inner_ty(),
            MatInstance::Mat4x3(m) => m.inner_ty(),
            MatInstance::Mat4x4(m) => m.inner_ty(),
        }
    }
}

impl Ty for PtrInstance {
    fn ty(&self) -> Type {
        Type::Ptr(Box::new(self.ty.clone()))
    }
}

impl Ty for RefInstance {
    fn ty(&self) -> Type {
        self.ty.clone()
    }
}

pub trait EvalTy {
    fn eval_ty(&self, ctx: &mut Context) -> Result<Type, ConstEvalError>;
}

impl<T: Ty> EvalTy for T {
    fn eval_ty(&self, _ctx: &mut Context) -> Result<Type, ConstEvalError> {
        Ok(self.ty())
    }
}

impl EvalTy for TypeExpression {
    fn eval_ty(&self, ctx: &mut Context) -> Result<Type, ConstEvalError> {
        match self.name.as_str() {
            "bool" => self
                .template_args
                .is_none()
                .then_some(Type::Bool)
                .ok_or_else(|| ConstEvalError::UnexpectedTemplate(Type::Bool)),
            "i32" => self
                .template_args
                .is_none()
                .then_some(Type::I32)
                .ok_or_else(|| ConstEvalError::UnexpectedTemplate(Type::I32)),
            "u32" => self
                .template_args
                .is_none()
                .then_some(Type::U32)
                .ok_or_else(|| ConstEvalError::UnexpectedTemplate(Type::U32)),
            "f32" => self
                .template_args
                .is_none()
                .then_some(Type::F32)
                .ok_or_else(|| ConstEvalError::UnexpectedTemplate(Type::F32)),
            "f16" => self
                .template_args
                .is_none()
                .then_some(Type::F16)
                .ok_or_else(|| ConstEvalError::UnexpectedTemplate(Type::F16)),
            "array" => self
                .template_args
                .as_ref()
                .map(|args| match args.as_slice() {
                    [e1, e2] => {
                        let ty = e1.eval(ctx)?.ty(); // TODO: check is valid array type
                        let n_ty = e2.eval(ctx)?.ty(); // TODO: check is concrete integer scalar.
                        Ok(Type::Array(Box::new(ty)))
                    }
                    [e] => {
                        let ty = e.eval(ctx)?.ty(); // TODO: check is valid array type
                        Ok(Type::Array(Box::new(ty)))
                    }
                    _ => Err(ConstEvalError::TemplateArgs("array")),
                })
                .unwrap_or_else(|| Err(ConstEvalError::MissingTemplate("array"))),
            "vec2" | "vec3" | "vec4" => self
                .template_args
                .as_ref()
                .map(|args| match args.as_slice() {
                    [e] => {
                        let n = self.name.chars().nth(3).unwrap().to_digit(10).unwrap() as u8;
                        let ty = e.eval(ctx)?.ty(); // TODO: check is numeric scalar
                        Ok(Type::Vec(n, Box::new(ty)))
                    }
                    _ => Err(ConstEvalError::TemplateArgs("vecN")),
                })
                .unwrap_or_else(|| Err(ConstEvalError::MissingTemplate("vec"))),
            "mat2x2" | "mat2x3" | "mat2x4" | "mat3x2" | "mat3x3" | "mat3x4" | "mat4x2"
            | "mat4x3" | "mat4x4" => self
                .template_args
                .as_ref()
                .map(|args| match args.as_slice() {
                    [e] => {
                        let c = self.name.chars().nth(3).unwrap().to_digit(10).unwrap() as u8;
                        let r = self.name.chars().nth(5).unwrap().to_digit(10).unwrap() as u8;
                        let ty = e.eval(ctx)?.ty(); // TODO: check is numeric scalar
                        Ok(Type::Mat(c, r, Box::new(ty)))
                    }
                    _ => Err(ConstEvalError::TemplateArgs("matCxR")),
                })
                .unwrap_or_else(|| Err(ConstEvalError::MissingTemplate("matCxR"))),
            _ => {
                if let Some(ty) = ctx.source.resolve_alias(&self.name) {
                    ty.eval_ty(ctx)
                } else {
                    let ty = Type::Struct(self.name.clone());
                    if self.template_args.is_some() {
                        Err(ConstEvalError::UnexpectedTemplate(ty))
                    } else if ctx.source.decl_struct(&self.name).is_none() {
                        Err(ConstEvalError::UnknownType(self.clone()))
                    } else {
                        Ok(ty)
                    }
                }
            }
        }
    }
}
