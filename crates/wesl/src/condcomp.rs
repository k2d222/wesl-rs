use std::collections::HashMap;

use crate::{visit::Visit, Diagnostic};
use itertools::Itertools;
use thiserror::Error;
use wgsl_parse::{syntax::*, Decorated};

/// Conditional translation error.
#[derive(Clone, Debug, Error)]
pub enum CondCompError {
    #[error("invalid feature flag: `{0}`")]
    InvalidFeatureFlag(String),
    #[error("missing feature flag: `{0}`")]
    MissingFeatureFlag(String),
    #[error("invalid if attribute expression: `{0}`")]
    InvalidExpression(Expression),
}

type E = crate::Error;

type Features = HashMap<String, bool>;

const EXPR_TRUE: Expression = Expression::Literal(LiteralExpression::Bool(true));
const EXPR_FALSE: Expression = Expression::Literal(LiteralExpression::Bool(false));

pub fn eval_attr(expr: &Expression, features: &Features) -> Result<Expression, CondCompError> {
    match expr {
        Expression::Literal(LiteralExpression::Bool(_)) => Ok(expr.clone()),
        Expression::Parenthesized(paren) => eval_attr(&paren.expression, features),
        Expression::Unary(unary) => {
            let operand = eval_attr(&unary.operand, features)?;
            match &unary.operator {
                UnaryOperator::LogicalNegation => {
                    let expr = if operand == EXPR_TRUE {
                        EXPR_FALSE.clone()
                    } else if operand == EXPR_FALSE {
                        EXPR_TRUE.clone()
                    } else {
                        expr.clone()
                    };
                    Ok(expr)
                }
                _ => Err(CondCompError::InvalidExpression(expr.clone())),
            }
        }
        Expression::Binary(binary) => {
            let left = eval_attr(&binary.left, features)?;
            let right = eval_attr(&binary.right, features)?;
            match &binary.operator {
                BinaryOperator::ShortCircuitOr => {
                    let expr = if left == EXPR_TRUE || right == EXPR_TRUE {
                        EXPR_TRUE.clone()
                    } else if left == EXPR_FALSE && right == EXPR_FALSE {
                        left // false
                    } else if left == EXPR_FALSE {
                        right
                    } else if right == EXPR_FALSE {
                        left
                    } else {
                        expr.clone()
                    };
                    Ok(expr)
                }
                BinaryOperator::ShortCircuitAnd => {
                    let expr = if left == EXPR_TRUE && right == EXPR_TRUE {
                        left // true
                    } else if left == EXPR_FALSE || right == EXPR_FALSE {
                        EXPR_FALSE.clone()
                    } else if left == EXPR_TRUE {
                        right
                    } else if right == EXPR_TRUE {
                        left
                    } else {
                        expr.clone()
                    };
                    Ok(expr)
                }
                _ => Err(CondCompError::InvalidExpression(expr.clone())),
            }
        }
        Expression::TypeOrIdentifier(ty) => {
            if ty.template_args.is_some() {
                return Err(CondCompError::InvalidFeatureFlag(ty.to_string()));
            }
            let feat = features.get(&*ty.ident.name());
            let expr = match feat {
                Some(true) => EXPR_TRUE.clone(),
                Some(false) => EXPR_FALSE.clone(),
                None => expr.clone(),
            };
            Ok(expr)
        }
        _ => Err(CondCompError::InvalidExpression(expr.clone())),
    }
}

fn eval_if_attr(
    opt_node: &mut Option<impl Decorated>,
    features: &Features,
) -> Result<(), CondCompError> {
    if let Some(node) = opt_node {
        let if_attr = node
            .attributes_mut()
            .iter_mut()
            .find_map(|attr| match attr {
                Attribute::If(expr) => Some(expr),
                _ => None,
            });

        if let Some(if_attr) = if_attr {
            let expr = eval_attr(if_attr, features)?;
            let keep = !(expr == EXPR_FALSE);
            if keep {
                **if_attr = expr;
            } else {
                *opt_node = None;
            }
        }
    }
    Ok(())
}

fn eval_if_attributes(nodes: &mut Vec<impl Decorated>, features: &Features) -> Result<(), E> {
    let retains = nodes
        .iter()
        .map(|node| {
            let if_attr = node.attributes().iter().find_map(|attr| match attr {
                Attribute::If(expr) => Some(expr),
                _ => None,
            });

            if let Some(expr) = if_attr {
                eval_attr(expr, features)
                    .map_err(|e| Diagnostic::from(e).with_span(expr.span().clone()))
            } else {
                Ok(EXPR_TRUE.clone())
            }
        })
        .collect::<Result<Vec<Expression>, Diagnostic<E>>>()?;

    let retains = nodes
        .iter_mut()
        .zip(retains)
        .map(|(node, expr)| {
            let if_attr = node
                .attributes_mut()
                .iter_mut()
                .find_map(|attr| match attr {
                    Attribute::If(expr) => Some(expr),
                    _ => None,
                });
            if let Some(if_attr) = if_attr {
                let keep = !(expr == EXPR_FALSE);
                **if_attr = expr;
                keep
            } else {
                true
            }
        })
        .collect_vec();

    let mut it = retains.iter();
    nodes.retain(|_| *it.next().unwrap());
    Ok(())
}

fn statement_eval_if_attributes(
    statements: &mut Vec<StatementNode>,
    features: &HashMap<String, bool>,
) -> Result<(), E> {
    fn rec_one(stmt: &mut StatementNode, feats: &HashMap<String, bool>) -> Result<(), E> {
        match stmt.node_mut() {
            Statement::Compound(stmt) => rec(&mut stmt.statements, feats)?,
            Statement::If(stmt) => {
                rec(&mut stmt.if_clause.body.statements, feats)?;
                for elif in &mut stmt.else_if_clauses {
                    rec(&mut elif.body.statements, feats)?;
                }
                if let Some(el) = &mut stmt.else_clause {
                    rec(&mut el.body.statements, feats)?;
                }
            }
            Statement::Switch(stmt) => {
                eval_if_attributes(&mut stmt.clauses, feats)?;
                for clause in &mut stmt.clauses {
                    rec(&mut clause.body.statements, feats)?;
                }
            }
            Statement::Loop(stmt) => {
                rec(&mut stmt.body.statements, feats)?;
                eval_if_attr(&mut stmt.continuing, feats)?;
                if let Some(cont) = &mut stmt.continuing {
                    rec(&mut cont.body.statements, feats)?;
                    eval_if_attr(&mut cont.break_if, feats)?;
                }
                rec(&mut stmt.body.statements, feats)?;
            }
            Statement::For(stmt) => {
                if let Some(init) = &mut stmt.initializer {
                    rec_one(&mut *init, feats)?
                }
                if let Some(updt) = &mut stmt.update {
                    rec_one(&mut *updt, feats)?
                }
                rec(&mut stmt.body.statements, feats)?
            }
            Statement::While(stmt) => rec(&mut stmt.body.statements, feats)?,
            _ => (),
        };
        Ok(())
    }
    fn rec(stats: &mut Vec<StatementNode>, feats: &HashMap<String, bool>) -> Result<(), E> {
        eval_if_attributes(stats, feats)?;
        for stmt in stats {
            rec_one(stmt, feats)?;
        }
        Ok(())
    }
    rec(statements, features)
}

pub fn run(wesl: &mut TranslationUnit, features: &Features) -> Result<(), E> {
    // 1. evaluate all if attributes

    #[cfg(feature = "imports")]
    eval_if_attributes(&mut wesl.imports, features)?;

    eval_if_attributes(&mut wesl.global_directives, features)?;
    eval_if_attributes(&mut wesl.global_declarations, features)?;

    let structs = wesl
        .global_declarations
        .iter_mut()
        .filter_map(|decl| match decl {
            wgsl_parse::syntax::GlobalDeclaration::Struct(decl) => Some(decl),
            _ => None,
        });
    for decl in structs {
        eval_if_attributes(&mut decl.members, features)
            .map_err(|e| Diagnostic::from(e).with_declaration(decl.ident.to_string()))?;
    }

    let functions = wesl
        .global_declarations
        .iter_mut()
        .filter_map(|decl| match decl {
            wgsl_parse::syntax::GlobalDeclaration::Function(decl) => Some(decl),
            _ => None,
        });
    for func in functions {
        eval_if_attributes(&mut func.parameters, features)
            .map_err(|e| Diagnostic::from(e).with_declaration(func.ident.to_string()))?;
        statement_eval_if_attributes(&mut func.body.statements, features)
            .map_err(|e| Diagnostic::from(e).with_declaration(func.ident.to_string()))?;
    }

    // 2. remove attributes that evaluate to true

    for attrs in Visit::<Attributes>::visit_mut(wesl) {
        attrs.retain(|attr| match attr {
            Attribute::If(expr) => **expr != EXPR_TRUE,
            _ => true,
        })
    }
    Ok(())
}
