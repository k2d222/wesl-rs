use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use derive_more::derive::From;
use itertools::Itertools;

use crate::consteval::Ty;

use super::{ConstEvalError, Context, Type};

#[derive(Clone, Debug, From, PartialEq)]
pub enum Instance {
    Literal(LiteralInstance),
    Struct(StructInstance),
    Array(ArrayInstance),
    Vec(VecInstance),
    Mat(MatInstance),
    Ptr(PtrInstance),
    Ref(RefInstance),
    Type(Type),
    Void,
}

impl Instance {
    pub fn view<'a>(&'a self, view: &'a MemView) -> Option<&Instance> {
        match view {
            MemView::Whole => Some(self),
            MemView::Member(m, v) => match self {
                Instance::Struct(s) => {
                    let inst = s.components.get(m)?;
                    inst.view(v)
                }
                _ => None,
            },
            MemView::Index(i, v) => match self {
                Instance::Array(a) => {
                    let inst = a.components.get(*i)?;
                    inst.view(v)
                }
                _ => None,
            },
        }
    }
    pub fn view_mut<'a>(&'a mut self, view: &'a MemView) -> Option<&mut Instance> {
        match view {
            MemView::Whole => Some(self),
            MemView::Member(m, v) => match self {
                Instance::Struct(s) => {
                    let inst = s.components.get_mut(m)?;
                    inst.view_mut(v)
                }
                _ => None,
            },
            MemView::Index(i, v) => match self {
                Instance::Array(a) => {
                    let inst = a.components.get_mut(*i)?;
                    inst.view_mut(v)
                }
                _ => None,
            },
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Instance::Array(a) => a.n(),
            Instance::Vec(v) => v.n() as usize,
            Instance::Mat(m) => m.c() as usize,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, From)]
pub enum LiteralInstance {
    Bool(bool),
    AbstractInt(i64),
    AbstractFloat(f64),
    I32(i32),
    U32(u32),
    F32(f32),
    #[from(skip)]
    F16(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructInstance {
    pub name: String,
    pub components: HashMap<String, Instance>,
}

impl StructInstance {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArrayInstance {
    pub components: Vec<Instance>,
}

impl ArrayInstance {
    pub(crate) fn new(components: Vec<Instance>) -> Self {
        debug_assert!(components.iter().map(|c| c.ty()).all_equal());
        Self { components }
    }

    pub fn n(&self) -> usize {
        self.components.len()
    }

    pub fn get(&self, i: usize) -> Option<&Instance> {
        self.components.get(i)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VecInner<const N: usize> {
    pub components: Vec<LiteralInstance>,
}

pub type Vec2 = VecInner<2>;
pub type Vec3 = VecInner<3>;
pub type Vec4 = VecInner<4>;

impl<const N: usize> VecInner<N> {
    pub fn new(components: Vec<LiteralInstance>) -> Self {
        assert!(components.len() == N);
        Self { components }
    }
    pub fn new_with_value(val: &LiteralInstance) -> Self {
        Self {
            components: Vec::from_iter((0..N).map(|_| val.clone())),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LiteralInstance> {
        self.components.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut LiteralInstance> {
        self.components.iter_mut()
    }
}

impl<const N: usize> From<Vec<LiteralInstance>> for VecInner<N> {
    fn from(components: Vec<LiteralInstance>) -> Self {
        assert!(components.len() == N);
        Self { components }
    }
}

#[derive(Clone, Debug, PartialEq, From)]
pub enum VecInstance {
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
}

impl VecInstance {
    pub(crate) fn new(components: Vec<LiteralInstance>) -> Self {
        match components.len() {
            2 => Self::Vec2(VecInner::new(components)),
            3 => Self::Vec3(VecInner::new(components)),
            4 => Self::Vec4(VecInner::new(components)),
            _ => panic!("VecInstance must have 2, 3 or 4 commponents"),
        }
    }

    pub fn n(&self) -> u8 {
        match self {
            VecInstance::Vec2(_) => 2,
            VecInstance::Vec3(_) => 3,
            VecInstance::Vec4(_) => 4,
        }
    }

    pub fn get(&self, i: usize) -> Option<&LiteralInstance> {
        match self {
            VecInstance::Vec2(v) => v.components.get(i),
            VecInstance::Vec3(v) => v.components.get(i),
            VecInstance::Vec4(v) => v.components.get(i),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LiteralInstance> {
        match self {
            Self::Vec2(v) => v.components.iter(),
            Self::Vec3(v) => v.components.iter(),
            Self::Vec4(v) => v.components.iter(),
        }
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut LiteralInstance> {
        match self {
            Self::Vec2(v) => v.components.iter_mut(),
            Self::Vec3(v) => v.components.iter_mut(),
            Self::Vec4(v) => v.components.iter_mut(),
        }
    }
}

impl From<Vec<LiteralInstance>> for VecInstance {
    fn from(components: Vec<LiteralInstance>) -> Self {
        match components.len() {
            2 => Self::Vec2(VecInner { components }),
            3 => Self::Vec2(VecInner { components }),
            4 => Self::Vec2(VecInner { components }),
            _ => panic!("number of VecInstance components must be 2, 3, 4"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatInner<const C: usize, const R: usize> {
    pub components: Vec<VecInner<R>>,
}

pub type Mat2x2 = MatInner<2, 2>;
pub type Mat2x3 = MatInner<2, 3>;
pub type Mat2x4 = MatInner<2, 4>;
pub type Mat3x2 = MatInner<3, 2>;
pub type Mat3x3 = MatInner<3, 3>;
pub type Mat3x4 = MatInner<3, 4>;
pub type Mat4x2 = MatInner<4, 2>;
pub type Mat4x3 = MatInner<4, 3>;
pub type Mat4x4 = MatInner<4, 4>;

impl<const C: usize, const R: usize> MatInner<C, R> {
    pub fn new(components: Vec<VecInner<R>>) -> Self {
        Self { components }
    }
    pub fn new_with_value(val: &LiteralInstance) -> Self {
        Self {
            components: Vec::from_iter((0..C).map(|_| VecInner::new_with_value(val))),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LiteralInstance> {
        self.components.iter().flat_map(|c| c.iter())
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut LiteralInstance> {
        self.components.iter_mut().flat_map(|c| c.iter_mut())
    }
}

impl<const C: usize, const R: usize> From<Vec<VecInner<R>>> for MatInner<C, R> {
    fn from(components: Vec<VecInner<R>>) -> Self {
        assert!(components.len() == C);
        Self { components }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MatInstance {
    Mat2x2(Mat2x2),
    Mat2x3(Mat2x3),
    Mat2x4(Mat2x4),
    Mat3x2(Mat3x2),
    Mat3x3(Mat3x3),
    Mat3x4(Mat3x4),
    Mat4x2(Mat4x2),
    Mat4x3(Mat4x3),
    Mat4x4(Mat4x4),
}

impl MatInstance {
    pub fn r(&self) -> u8 {
        match self {
            MatInstance::Mat2x2(_) => 2,
            MatInstance::Mat2x3(_) => 3,
            MatInstance::Mat2x4(_) => 4,
            MatInstance::Mat3x2(_) => 2,
            MatInstance::Mat3x3(_) => 3,
            MatInstance::Mat3x4(_) => 4,
            MatInstance::Mat4x2(_) => 2,
            MatInstance::Mat4x3(_) => 3,
            MatInstance::Mat4x4(_) => 4,
        }
    }
    pub fn c(&self) -> u8 {
        match self {
            MatInstance::Mat2x2(_) => 2,
            MatInstance::Mat2x3(_) => 2,
            MatInstance::Mat2x4(_) => 2,
            MatInstance::Mat3x2(_) => 3,
            MatInstance::Mat3x3(_) => 3,
            MatInstance::Mat3x4(_) => 3,
            MatInstance::Mat4x2(_) => 4,
            MatInstance::Mat4x3(_) => 4,
            MatInstance::Mat4x4(_) => 4,
        }
    }

    pub fn col(&self, i: usize) -> Option<VecInstance> {
        match self {
            MatInstance::Mat2x2(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat2x3(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat2x4(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat3x2(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat3x3(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat3x4(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat4x2(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat4x3(m) => m.components.get(i).map(|c| c.clone().into()),
            MatInstance::Mat4x4(m) => m.components.get(i).map(|c| c.clone().into()),
        }
    }

    pub fn get(&self, i: usize, j: usize) -> Option<&LiteralInstance> {
        match self {
            MatInstance::Mat2x2(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat2x3(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat2x4(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat3x2(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat3x3(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat3x4(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat4x2(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat4x3(m) => m.components.get(i).and_then(|c| c.components.get(j)),
            MatInstance::Mat4x4(m) => m.components.get(i).and_then(|c| c.components.get(j)),
        }
    }

    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = &LiteralInstance> + 'a> {
        match self {
            MatInstance::Mat2x2(m) => Box::new(m.iter()),
            MatInstance::Mat2x3(m) => Box::new(m.iter()),
            MatInstance::Mat2x4(m) => Box::new(m.iter()),
            MatInstance::Mat3x2(m) => Box::new(m.iter()),
            MatInstance::Mat3x3(m) => Box::new(m.iter()),
            MatInstance::Mat3x4(m) => Box::new(m.iter()),
            MatInstance::Mat4x2(m) => Box::new(m.iter()),
            MatInstance::Mat4x3(m) => Box::new(m.iter()),
            MatInstance::Mat4x4(m) => Box::new(m.iter()),
        }
    }
    pub fn iter_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = &mut LiteralInstance> + 'a> {
        match self {
            MatInstance::Mat2x2(m) => Box::new(m.iter_mut()),
            MatInstance::Mat2x3(m) => Box::new(m.iter_mut()),
            MatInstance::Mat2x4(m) => Box::new(m.iter_mut()),
            MatInstance::Mat3x2(m) => Box::new(m.iter_mut()),
            MatInstance::Mat3x3(m) => Box::new(m.iter_mut()),
            MatInstance::Mat3x4(m) => Box::new(m.iter_mut()),
            MatInstance::Mat4x2(m) => Box::new(m.iter_mut()),
            MatInstance::Mat4x3(m) => Box::new(m.iter_mut()),
            MatInstance::Mat4x4(m) => Box::new(m.iter_mut()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PtrInstance {
    pub ty: Type,
    pub view: MemView,
    pub ptr: Rc<RefCell<Instance>>,
}

impl From<RefInstance> for PtrInstance {
    fn from(r: RefInstance) -> Self {
        Self {
            ty: r.ty.clone(),
            view: r.view.clone(),
            ptr: r.ptr.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefInstance {
    pub ty: Type,
    pub view: MemView,
    pub ptr: Rc<RefCell<Instance>>,
}

impl RefInstance {
    pub fn new(ptr: Rc<RefCell<Instance>>) -> Self {
        let ty = ptr.borrow().ty();
        Self {
            ty,
            view: MemView::Whole,
            ptr,
        }
    }
}

impl From<PtrInstance> for RefInstance {
    fn from(p: PtrInstance) -> Self {
        Self {
            ty: p.ty.clone(),
            view: p.view.clone(),
            ptr: p.ptr.clone(),
        }
    }
}

pub struct RefView<'a> {
    r: Ref<'a, Instance>,
    v: &'a MemView,
}

impl<'a> Deref for RefView<'a> {
    type Target = Instance;

    fn deref(&self) -> &Self::Target {
        self.r.view(self.v).expect("invalid reference")
    }
}

pub struct RefViewMut<'a> {
    r: RefMut<'a, Instance>,
    v: &'a MemView,
}

impl<'a> Deref for RefViewMut<'a> {
    type Target = Instance;

    fn deref(&self) -> &Self::Target {
        self.r.view(self.v).expect("invalid reference")
    }
}

impl<'a> DerefMut for RefViewMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.r.view_mut(self.v).expect("invalid reference")
    }
}

impl RefInstance {
    pub fn view_member(&self, member: String) -> Option<Self> {
        let mut view = self.view.clone();
        view.append_member(member);
        let inst = self.ptr.borrow();
        let inst = inst.view(&self.view)?;
        Some(Self {
            ty: inst.ty(),
            view,
            ptr: self.ptr.clone(),
        })
    }
    pub fn view_index(&self, index: usize) -> Option<Self> {
        let mut view = self.view.clone();
        view.append_index(index);
        let inst = self.ptr.borrow();
        let inst = inst.view(&self.view)?;
        Some(Self {
            ty: inst.ty(),
            view,
            ptr: self.ptr.clone(),
        })
    }

    pub fn deref_inst<'a>(&'a self) -> RefView<'a> {
        let r = self.ptr.borrow();
        RefView { r, v: &self.view }
    }

    pub fn deref_inst_mut<'a>(&'a mut self) -> RefViewMut<'a> {
        let r = self.ptr.borrow_mut();
        RefViewMut { r, v: &self.view }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemView {
    Whole,
    Member(String, Box<MemView>),
    Index(usize, Box<MemView>),
}

impl MemView {
    pub fn append_member(&mut self, m: String) {
        match self {
            MemView::Whole => *self = MemView::Member(m, Box::new(MemView::Whole)),
            MemView::Member(_, v) | MemView::Index(_, v) => v.append_member(m),
        }
    }
    pub fn append_index(&mut self, i: usize) {
        match self {
            MemView::Whole => *self = MemView::Index(i, Box::new(MemView::Whole)),
            MemView::Member(_, v) | MemView::Index(_, v) => v.append_index(i),
        }
    }
}

// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct Address {
//     pub ptr: usize,
//     pub view: MemView,
// }

// impl Address {
//     pub fn new(ptr: usize) -> Self {
//         Self {
//             ptr,
//             view: MemView::Whole,
//         }
//     }
// }
