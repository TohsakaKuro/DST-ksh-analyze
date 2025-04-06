use crate::types::{Variable, VariableScope};
use glsl_lang::{ast, parse::Parse};
use std::collections::HashSet;

pub fn parse_glsl_uniforms(content: &str) -> Result<Vec<Variable>, Box<dyn std::error::Error>> {
    let ast::TranslationUnit(declarations) =
        ast::TranslationUnit::parse::<glsl_lang::parse::DefaultLexer>(&content)?;

    let mut uniforms = collect_uniform_declarations(&declarations)?;
    let used_vars = collect_used_variables(&declarations);
    uniforms.retain(|u| used_vars.contains(&u.name));

    Ok(uniforms)
}

// 收集 Uniform 声明
fn collect_uniform_declarations(
    declarations: &[ast::ExternalDeclaration],
) -> Result<Vec<Variable>, Box<dyn std::error::Error>> {
    let mut uniforms = vec![];

    for decl in declarations {
        let ast::ExternalDeclarationData::Declaration(decl) = &decl.content else {
            continue;
        };
        let ast::DeclarationData::InitDeclaratorList(decl_list) = &decl.content else {
            continue;
        };
        let head = &decl_list.content.head;

        // 检查是否是 uniform 类型
        let is_uniform = head.ty.content.qualifier.as_ref()
            .and_then(|q| q.qualifiers.first())
            .map(|q| matches!(&q.content, ast::TypeQualifierSpecData::Storage(s) if s.content == ast::StorageQualifierData::Uniform))
            .unwrap_or(false);

        if !is_uniform {
            continue;
        }

        let name = head
            .name
            .as_ref()
            .map(|n| n.content.as_str())
            .ok_or("Uniform 缺少名称")?
            .to_string();

        // 提取类型和数组长度
        let type_ = &head.ty.ty.ty.content;

        let array_length = head
            .array_specifier
            .as_ref()
            .and_then(|a| a.content.dimensions.first())
            .and_then(|d| {
                if let ast::ArraySpecifierDimensionData::ExplicitlySized(es) = &d.content {
                    if let ast::ExprData::IntConst(size) = &es.content {
                        Some(*size as u32)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
        let mut variable = Variable::new();
        variable.name = name;
        variable.r#type = type_.clone();
        variable.array_length = array_length;
        variable.default_data = vec![];
        variable.scope = VariableScope::UNIFORM;
        uniforms.push(variable);
    }

    Ok(uniforms)
}

// 收集使用的变量
fn collect_used_variables(declarations: &[ast::ExternalDeclaration]) -> HashSet<String> {
    let mut used_vars = HashSet::new();
    for decl in declarations {
        if let ast::ExternalDeclarationData::FunctionDefinition(f) = &decl.content {
            for stmt in &f.content.statement.content.statement_list {
                walk_statement(&stmt, &mut used_vars);
            }
        }
    }
    used_vars
}

fn walk_condition(condition: &ast::Condition, vars: &mut HashSet<String>) {
    match &condition.content {
        ast::ConditionData::Expr(expr) => {
            walk_expr(expr, vars);
        }
        ast::ConditionData::Assignment(_, _, _) => {
            // TODO: 处理赋值表达式
            unimplemented!()
        }
    }
}

fn walk_initializer(init: &ast::Initializer, used_vars: &mut HashSet<String>) {
    match &init.content {
        ast::InitializerData::Simple(expr) => {
            walk_expr(expr, used_vars);
        }
        ast::InitializerData::List(init_list) => {
            for item in init_list {
                walk_initializer(item, used_vars);
            }
        }
    }
}

fn walk_statement(stmt: &ast::Statement, vars: &mut HashSet<String>) {
    match &stmt.content {
        ast::StatementData::Declaration(decl) => match &decl.content {
            ast::DeclarationData::InitDeclaratorList(decl_list) => {
                if let Some(init) = &decl_list.content.head.content.initializer {
                    walk_initializer(init, vars);
                }
                for decl in &decl_list.content.tail {
                    if let Some(init) = &decl.content.initializer {
                        walk_initializer(init, vars);
                    }
                }
            }
            _ => {}
        },
        ast::StatementData::Expression(expr_stmt) => {
            if let Some(expr) = &expr_stmt.0 {
                walk_expr(expr, vars);
            }
        }
        ast::StatementData::Selection(selection_stmt) => {
            walk_expr(&selection_stmt.cond, vars);
            match &selection_stmt.rest.content {
                ast::SelectionRestStatementData::Statement(then_stmt) => {
                    walk_statement(&then_stmt, vars);
                }
                ast::SelectionRestStatementData::Else(body_stmt, next_stmt) => {
                    walk_statement(body_stmt, vars);
                    walk_statement(next_stmt, vars);
                }
            }
        }
        ast::StatementData::Switch(switch) => {
            walk_expr(&switch.head, vars);
            for stmt_node in &switch.body {
                walk_statement(&stmt_node, vars);
            }
        }
        ast::StatementData::CaseLabel(case) => {
            if let ast::CaseLabelData::Case(expr) = &case.content {
                walk_expr(expr, vars);
            }
        }
        ast::StatementData::Iteration(iteration_stmt) => match &iteration_stmt.content {
            ast::IterationStatementData::While(condition, stmt) => {
                walk_condition(condition, vars);
                walk_statement(stmt, vars);
            }
            ast::IterationStatementData::For(for_in_stmt, for_rest_stmt, stmt) => {
                if let ast::ForInitStatementData::Expression(expr) = &for_in_stmt.content {
                    if let Some(expr) = expr {
                        walk_expr(expr, vars);
                    }
                }
                if let Some(stmt) = &for_rest_stmt.condition {
                    walk_condition(stmt, vars);
                }
                if let Some(expr) = &for_rest_stmt.post_expr {
                    walk_expr(expr, vars);
                }
                walk_statement(stmt, vars);
            }
            ast::IterationStatementData::DoWhile(stmt, expr) => {
                walk_statement(stmt, vars);
                walk_expr(expr, vars);
            }
        },
        ast::StatementData::Jump(jump) => {
            if let ast::JumpStatementData::Return(expr) = &jump.content {
                if let Some(expr) = expr {
                    walk_expr(expr, vars);
                }
            }
        }
        ast::StatementData::Compound(compound_stmt) => {
            for stmt_node in &compound_stmt.statement_list {
                walk_statement(&stmt_node, vars);
            }
        }
    }
}

fn walk_expr(expr: &ast::Expr, vars: &mut HashSet<String>) {
    match &expr.content {
        ast::ExprData::Variable(name) => {
            vars.insert(name.content.as_str().to_string());
        }
        ast::ExprData::Unary(_, expr) => {
            walk_expr(expr, vars);
        }
        ast::ExprData::Binary(_, left, right) => {
            walk_expr(left, vars);
            walk_expr(right, vars);
        }
        ast::ExprData::Ternary(cond, left, right) => {
            walk_expr(cond, vars);
            walk_expr(left, vars);
            walk_expr(right, vars);
        }
        ast::ExprData::Assignment(left, _, right) => {
            walk_expr(left, vars);
            walk_expr(right, vars);
        }
        ast::ExprData::Bracket(arr, idx) => {
            walk_expr(arr, vars);
            walk_expr(idx, vars);
        }
        ast::ExprData::FunCall(_, args) => {
            for arg in args {
                walk_expr(arg, vars);
            }
        }
        ast::ExprData::Dot(struct_expr, _) => {
            walk_expr(struct_expr, vars);
        }
        ast::ExprData::PostInc(expr) => {
            walk_expr(expr, vars);
        }
        ast::ExprData::PostDec(expr) => {
            walk_expr(expr, vars);
        }
        ast::ExprData::Comma(a, b) => {
            walk_expr(a, vars);
            walk_expr(b, vars);
        }
        _ => {}
    }
}
