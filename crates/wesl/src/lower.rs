use crate::{visit::Visit, Error};

use wgsl_parse::syntax::*;

/// Performs conversions on the final syntax tree to make it more compatible with naga,
/// catch errors early and perform optimizations.
pub fn lower(wesl: &mut TranslationUnit, _keep: &[String]) -> Result<(), Error> {
    #[cfg(feature = "imports")]
    wesl.imports.clear();

    for attrs in Visit::<Attributes>::visit_mut(wesl) {
        attrs.retain(|attr| {
            !matches!(attr, 
            Attribute::Custom(CustomAttribute { name, .. }) if name == "generic")
        })
    }

    #[cfg(not(feature = "eval"))]
    {
        // these are redundant with eval::lower.
        remove_type_aliases(wesl);
        remove_global_consts(wesl);
    }
    #[cfg(feature = "eval")]
    {
        use crate::eval::{make_explicit_conversions, mark_functions_const, Context, Lower};
        use crate::Diagnostic;
        mark_functions_const(wesl);

        // we want to drop wesl2 at the end of the block for idents use_count
        {
            let wesl2 = wesl.clone();
            let mut ctx = Context::new(&wesl2);
            make_explicit_conversions(wesl, &mut ctx)
                .map_err(|e| Diagnostic::from(e).with_ctx(&ctx))?;
            wesl.lower(&mut ctx)
                .map_err(|e| Diagnostic::from(e).with_ctx(&ctx))?;
        }

        // lowering sometimes makes const function unused, so we remove them if not in keep list.
        wesl.global_declarations.retain_mut(|decl| {
            if let GlobalDeclaration::Function(decl) = decl {
                if decl.attributes.contains(&Attribute::Const)
                    && !_keep.contains(&*decl.ident.name())
                    // TODO: there may be a race cond here
                    && decl.ident.use_count() == 1
                {
                    false
                } else {
                    decl.attributes.retain(|attr| *attr != Attribute::Const);
                    true
                }
            } else {
                true
            }
        });
    }
    Ok(())
}

/// Eliminate all type aliases.
/// Naga doesn't like this: `alias T = u32; vec<T>`
#[allow(unused)]
fn remove_type_aliases(wesl: &mut TranslationUnit) {
    let take_next_alias = |wesl: &mut TranslationUnit| {
        let index = wesl
            .global_declarations
            .iter()
            .position(|decl| matches!(decl, GlobalDeclaration::TypeAlias(_)));
        index.map(|index| {
            let decl = wesl.global_declarations.swap_remove(index);
            match decl {
                GlobalDeclaration::TypeAlias(alias) => alias,
                _ => unreachable!(),
            }
        })
    };

    while let Some(mut alias) = take_next_alias(wesl) {
        // we rename the alias and all references to its type expression,
        // and drop the alias declaration.
        alias.ident.rename(format!("{}", alias.ty));
    }
}

/// Eliminate all const-declarations.
///
/// Replace usages of the const-declaration with its expression.
///
/// # Panics
/// panics if the const-declaration is ill-formed, i.e. has no initializer.
#[allow(unused)]
fn remove_global_consts(wesl: &mut TranslationUnit) {
    let take_next_const = |wesl: &mut TranslationUnit| {
        let index = wesl.global_declarations.iter().position(|decl| {
            matches!(
                decl,
                GlobalDeclaration::Declaration(Declaration {
                    kind: DeclarationKind::Const,
                    ..
                })
            )
        });
        index.map(|index| {
            let decl = wesl.global_declarations.swap_remove(index);
            match decl {
                GlobalDeclaration::Declaration(d) => d,
                _ => unreachable!(),
            }
        })
    };

    while let Some(mut decl) = take_next_const(wesl) {
        // we rename the const and all references to its expression in parentheses,
        // and drop the const declaration.
        decl.ident
            .rename(format!("({})", decl.initializer.unwrap()));
    }
}

// /// Eliminate most uses of AbstractInt and AbstractFloat.
// /// Naga doesn't like automatic type conversions in several places:
// /// * return statements
// /// * function calls
// /// * shift when lhs is abstract
// /// * ...probably more?
// trait MakeExplicit {
//     fn make_explicit(&mut self, scope: &mut Scope);
// }

// impl MakeExplicit for TranslationUnit {
//     fn make_explicit(&mut self, scope: &mut Scope) {
//         for decl in &mut self.global_declarations {
//             match decl {
//                 GlobalDeclaration::Void => todo!(),
//                 GlobalDeclaration::Declaration(decl) => decl.make_explicit(),
//                 GlobalDeclaration::TypeAlias(decl) => decl.make_explicit(),
//                 GlobalDeclaration::Struct(decl) => decl.make_explicit(),
//                 GlobalDeclaration::Function(decl) => decl.make_explicit(),
//                 GlobalDeclaration::ConstAssert(decl) => decl.make_explicit(),
//             }
//         }
//     }
// }
