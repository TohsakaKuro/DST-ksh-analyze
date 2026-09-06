use crate::types::{KshUniformType, UniformDeclaration};
use glsl_lang::{ast, parse::Parse};
use std::collections::HashSet;

type ParseResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn parse_glsl_uniforms(content: &str) -> ParseResult<Vec<UniformDeclaration>> {
    let translation_unit = ast::TranslationUnit::parse::<glsl_lang::parse::DefaultLexer>(content)?;

    let mut uniforms = collect_uniform_declarations(&translation_unit.0)?;
    let used_vars = collect_used_variables(&translation_unit);
    uniforms.retain(|uniform| used_vars.contains(&uniform.name));

    Ok(uniforms)
}

fn collect_uniform_declarations(
    declarations: &[ast::ExternalDeclaration],
) -> ParseResult<Vec<UniformDeclaration>> {
    let mut uniforms = Vec::new();
    let mut names = HashSet::new();

    for declaration in declarations {
        let ast::ExternalDeclarationData::Declaration(declaration) = &declaration.content else {
            continue;
        };
        let ast::DeclarationData::InitDeclaratorList(declarators) = &declaration.content else {
            continue;
        };
        let head = &declarators.content.head;

        let is_uniform = head
            .ty
            .content
            .qualifier
            .as_ref()
            .map(|qualifier| {
                qualifier.content.qualifiers.iter().any(|specifier| {
                    matches!(
                        &specifier.content,
                        ast::TypeQualifierSpecData::Storage(storage)
                            if storage.content == ast::StorageQualifierData::Uniform
                    )
                })
            })
            .unwrap_or(false);

        if !is_uniform {
            continue;
        }

        let uniform_type = &head.ty.content.ty.content.ty.content;
        let type_array = head.ty.content.ty.content.array_specifier.as_ref();
        let head_name = head
            .content
            .name
            .as_ref()
            .ok_or("uniform 声明缺少变量名")?
            .content
            .as_str();

        push_uniform(
            &mut uniforms,
            &mut names,
            head_name,
            uniform_type,
            type_array,
            head.content.array_specifier.as_ref(),
            head.content.initializer.as_ref(),
        )?;

        for tail in &declarators.content.tail {
            let identifier = &tail.content.ident.content;
            push_uniform(
                &mut uniforms,
                &mut names,
                identifier.ident.content.as_str(),
                uniform_type,
                type_array,
                identifier.array_spec.as_ref(),
                tail.content.initializer.as_ref(),
            )?;
        }
    }

    Ok(uniforms)
}

fn push_uniform(
    uniforms: &mut Vec<UniformDeclaration>,
    names: &mut HashSet<String>,
    name: &str,
    uniform_type: &ast::TypeSpecifierNonArrayData,
    type_array: Option<&ast::ArraySpecifier>,
    declarator_array: Option<&ast::ArraySpecifier>,
    initializer: Option<&ast::Initializer>,
) -> ParseResult<()> {
    if !names.insert(name.to_string()) {
        return Err(format!("uniform `{name}` 被重复声明").into());
    }

    if type_array.is_some() && declarator_array.is_some() {
        return Err(format!("uniform `{name}` 使用了多个数组维度；KSH 只支持一维数组").into());
    }

    let (array_count, is_array) = match type_array.or(declarator_array) {
        Some(array) => (parse_array_length(name, array)?, true),
        None => (1, false),
    };
    let uniform_type = KshUniformType::from_glsl(uniform_type)
        .map_err(|error| format!("uniform `{name}` 的类型不受 KSH 支持: {error}"))?;
    let default_data = parse_default_data(name, uniform_type, is_array, initializer)?;

    uniforms.push(UniformDeclaration {
        name: name.to_string(),
        uniform_type,
        array_count,
        is_array,
        default_data,
    });

    Ok(())
}

fn parse_default_data(
    name: &str,
    uniform_type: KshUniformType,
    is_array: bool,
    initializer: Option<&ast::Initializer>,
) -> ParseResult<Option<Vec<u32>>> {
    if uniform_type.is_sampler() {
        if initializer.is_some() {
            return Err(format!("sampler uniform `{name}` 不能包含初始化值").into());
        }
        return Ok(None);
    }
    // The official compiler emits no default block for numeric arrays, even when initialized.
    if is_array {
        return Ok(None);
    }

    let width = uniform_type.default_data_length().ok_or_else(|| {
        format!(
            "uniform `{name}` 的类型 {} 没有已确认的默认值宽度",
            uniform_type.name()
        )
    })?;
    let Some(initializer) = initializer else {
        return Ok(None);
    };
    let ast::InitializerData::Simple(expression) = &initializer.content else {
        return Err(format!("uniform `{name}` 的初始化值必须是数值字面量或数值类型构造器").into());
    };

    if let Some(constructor_type) = expression_constructor_type(expression) {
        if constructor_type != uniform_type {
            return Err(format!(
                "uniform `{name}` 的初始化构造器类型 {} 与声明类型 {} 不一致",
                constructor_type.name(),
                uniform_type.name()
            )
            .into());
        }
    }

    let values = evaluate_initializer(expression)
        .map_err(|error| format!("uniform `{name}` 的初始化值无法写入 KSH: {error}"))?;
    if values.len() != width {
        return Err(format!(
            "uniform `{name}` 的初始化值包含 {} 个分量，类型 {} 需要 {width} 个",
            values.len(),
            uniform_type.name()
        )
        .into());
    }

    Ok(Some(
        values
            .into_iter()
            .map(|value| normalize_component(uniform_type, value).to_bits())
            .collect(),
    ))
}

fn evaluate_initializer(expression: &ast::Expr) -> Result<Vec<f32>, String> {
    match &expression.content {
        ast::ExprData::FloatConst(value) => Ok(vec![*value]),
        ast::ExprData::DoubleConst(value) => Ok(vec![*value as f32]),
        ast::ExprData::IntConst(value) => Ok(vec![*value as f32]),
        ast::ExprData::UIntConst(value) => Ok(vec![*value as f32]),
        ast::ExprData::Unary(operator, operand) => {
            if matches!(&operand.content, ast::ExprData::UIntConst(_)) {
                return Err("无符号整数字面量不能使用一元正号或负号".to_string());
            }
            let values = evaluate_initializer(operand)?;
            match operator.content {
                ast::UnaryOpData::Add => Ok(values),
                ast::UnaryOpData::Minus => Ok(values.into_iter().map(|value| -value).collect()),
                _ => Err("只支持初始化常量的一元正号和负号".to_string()),
            }
        }
        ast::ExprData::FunCall(identifier, arguments) => {
            let ast::FunIdentifierData::TypeSpecifier(specifier) = &identifier.content else {
                return Err("只支持内建数值类型构造器，不支持函数调用".to_string());
            };
            if specifier.content.array_specifier.is_some() {
                return Err("不支持数组类型构造器".to_string());
            }
            let constructor_type = KshUniformType::from_glsl(&specifier.content.ty.content)
                .map_err(|_| "构造器类型不受 KSH 支持".to_string())?;
            evaluate_constructor(constructor_type, arguments)
        }
        _ => Err("只支持数值字面量、一元正负号和数值类型构造器".to_string()),
    }
}

fn expression_constructor_type(expression: &ast::Expr) -> Option<KshUniformType> {
    match &expression.content {
        ast::ExprData::FunCall(identifier, _) => {
            let ast::FunIdentifierData::TypeSpecifier(specifier) = &identifier.content else {
                return None;
            };
            if specifier.content.array_specifier.is_some() {
                return None;
            }
            KshUniformType::from_glsl(&specifier.content.ty.content).ok()
        }
        ast::ExprData::Unary(operator, operand)
            if matches!(
                operator.content,
                ast::UnaryOpData::Add | ast::UnaryOpData::Minus
            ) =>
        {
            expression_constructor_type(operand)
        }
        _ => None,
    }
}

fn evaluate_constructor(
    constructor_type: KshUniformType,
    arguments: &[ast::Expr],
) -> Result<Vec<f32>, String> {
    if constructor_type.is_sampler() {
        return Err("sampler 不是数值构造器".to_string());
    }
    let width = constructor_type
        .default_data_length()
        .ok_or_else(|| "构造器类型没有已确认的默认值宽度".to_string())?;
    let mut values = Vec::new();
    for argument in arguments {
        if matrix_dimensions(constructor_type).is_some() {
            if let Some(argument_type) = expression_constructor_type(argument) {
                if matrix_dimensions(argument_type).is_some() && argument_type != constructor_type {
                    return Err(format!(
                        "不支持从 {} 到 {} 的异型矩阵构造器转换",
                        argument_type.name(),
                        constructor_type.name()
                    ));
                }
            }
        }
        values.extend(evaluate_initializer(argument)?);
    }

    if values.len() == width {
        return Ok(values
            .into_iter()
            .map(|value| normalize_component(constructor_type, value))
            .collect());
    }
    if values.len() != 1 {
        return Err(format!(
            "{} 构造器得到 {} 个分量，需要 1 个或 {width} 个",
            constructor_type.name(),
            values.len()
        ));
    }

    let value = normalize_component(constructor_type, values[0]);
    if let Some((columns, rows)) = matrix_dimensions(constructor_type) {
        let mut matrix = vec![0.0; width];
        for diagonal in 0..columns.min(rows) {
            matrix[diagonal * rows + diagonal] = value;
        }
        Ok(matrix)
    } else {
        Ok(vec![value; width])
    }
}

fn normalize_component(uniform_type: KshUniformType, value: f32) -> f32 {
    if matches!(
        uniform_type,
        KshUniformType::Int | KshUniformType::IVec2 | KshUniformType::IVec3 | KshUniformType::IVec4
    ) {
        (value as i32) as f32
    } else {
        value
    }
}

fn matrix_dimensions(uniform_type: KshUniformType) -> Option<(usize, usize)> {
    match uniform_type {
        KshUniformType::Mat2 => Some((2, 2)),
        KshUniformType::Mat2x3 => Some((2, 3)),
        KshUniformType::Mat2x4 => Some((2, 4)),
        KshUniformType::Mat3x2 => Some((3, 2)),
        KshUniformType::Mat3 => Some((3, 3)),
        KshUniformType::Mat3x4 => Some((3, 4)),
        KshUniformType::Mat4x2 => Some((4, 2)),
        KshUniformType::Mat4x3 => Some((4, 3)),
        KshUniformType::Mat4 => Some((4, 4)),
        _ => None,
    }
}

fn parse_array_length(name: &str, array: &ast::ArraySpecifier) -> ParseResult<u32> {
    let dimensions = &array.content.dimensions;
    if dimensions.len() != 1 {
        return Err(format!(
            "uniform `{name}` 使用了 {} 个数组维度；KSH 只支持一维数组",
            dimensions.len()
        )
        .into());
    }

    let dimension = dimensions
        .first()
        .ok_or_else(|| format!("uniform `{name}` 的数组声明缺少维度"))?;
    let ast::ArraySpecifierDimensionData::ExplicitlySized(expression) = &dimension.content else {
        return Err(format!("uniform `{name}` 使用了未指定长度的数组；KSH 需要明确长度").into());
    };

    match &expression.content {
        ast::ExprData::IntConst(length) if *length > 0 => Ok(*length as u32),
        ast::ExprData::UIntConst(length) if *length > 0 => Ok(*length),
        ast::ExprData::IntConst(length) => {
            Err(format!("uniform `{name}` 的数组长度必须为正整数，实际为 {length}").into())
        }
        ast::ExprData::UIntConst(length) => {
            Err(format!("uniform `{name}` 的数组长度必须为正整数，实际为 {length}").into())
        }
        _ => Err(format!(
            "uniform `{name}` 的数组长度必须是正整数字面量或能由预处理器展开为该字面量的宏"
        )
        .into()),
    }
}

#[derive(Default)]
struct UsedVariableCollector {
    names: HashSet<String>,
    scopes: Vec<HashSet<String>>,
}

impl UsedVariableCollector {
    fn walk_translation_unit(&mut self, translation_unit: &ast::TranslationUnit) {
        for declaration in &translation_unit.0 {
            match &declaration.content {
                ast::ExternalDeclarationData::FunctionDefinition(definition) => {
                    self.walk_function_definition(definition);
                }
                ast::ExternalDeclarationData::Declaration(declaration) => {
                    self.walk_declaration(declaration, false);
                }
                ast::ExternalDeclarationData::Preprocessor(_) => {}
            }
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        let scope = self.scopes.pop();
        debug_assert!(scope.is_some());
    }

    fn bind(&mut self, identifier: &ast::Identifier) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(identifier.content.as_str().to_string());
        }
    }

    fn is_shadowed(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn reference(&mut self, identifier: &ast::Identifier) {
        let name = identifier.content.as_str();
        if !self.is_shadowed(name) {
            self.names.insert(identifier.content.as_str().to_string());
        }
    }

    fn walk_function_definition(&mut self, definition: &ast::FunctionDefinition) {
        let prototype = &definition.content.prototype;
        self.walk_function_prototype_types(prototype);

        self.push_scope();
        for parameter in &prototype.content.parameters {
            if let ast::FunctionParameterDeclarationData::Named(_, declarator) = &parameter.content
            {
                self.bind(&declarator.content.ident.content.ident);
            }
        }
        // Function parameters and top-level body declarations share the function scope.
        self.walk_compound_statement(&definition.content.statement);
        self.pop_scope();
    }

    fn walk_function_prototype_types(&mut self, prototype: &ast::FunctionPrototype) {
        self.walk_fully_specified_type(&prototype.content.ty);
        for parameter in &prototype.content.parameters {
            match &parameter.content {
                ast::FunctionParameterDeclarationData::Named(qualifier, declarator) => {
                    if let Some(qualifier) = qualifier {
                        self.walk_type_qualifier(qualifier);
                    }
                    self.walk_type_specifier(&declarator.content.ty);
                    self.walk_array_specifier(declarator.content.ident.content.array_spec.as_ref());
                }
                ast::FunctionParameterDeclarationData::Unnamed(qualifier, ty) => {
                    if let Some(qualifier) = qualifier {
                        self.walk_type_qualifier(qualifier);
                    }
                    self.walk_type_specifier(ty);
                }
            }
        }
    }

    fn walk_declaration(&mut self, declaration: &ast::Declaration, bind_locals: bool) {
        match &declaration.content {
            ast::DeclarationData::FunctionPrototype(prototype) => {
                self.walk_function_prototype_types(prototype);
            }
            ast::DeclarationData::InitDeclaratorList(declarators) => {
                self.walk_init_declarator_list(declarators, bind_locals);
            }
            ast::DeclarationData::Precision(_, ty) => self.walk_type_specifier(ty),
            ast::DeclarationData::Block(block) => {
                self.walk_block(block);
                // Named block instances have an unambiguous binding. Anonymous block members
                // intentionally remain unbound here, yielding a conservative false positive.
                if bind_locals {
                    if let Some(identifier) = &block.content.identifier {
                        self.bind(&identifier.content.ident);
                    }
                }
            }
            ast::DeclarationData::TypeOnly(qualifier) => {
                self.walk_type_qualifier(qualifier);
            }
            ast::DeclarationData::Invariant(_) => {}
        }
    }

    fn walk_init_declarator_list(
        &mut self,
        declarators: &ast::InitDeclaratorList,
        bind_locals: bool,
    ) {
        let head = &declarators.content.head;
        self.walk_fully_specified_type(&head.content.ty);
        self.walk_array_specifier(head.content.array_specifier.as_ref());
        if let Some(initializer) = &head.content.initializer {
            self.walk_initializer(initializer);
        }
        // GLSL initializers are resolved before the new binding enters scope. Processing the
        // declarators in order also lets later comma-separated initializers see earlier names.
        if bind_locals {
            if let Some(name) = &head.content.name {
                self.bind(name);
            }
        }

        for tail in &declarators.content.tail {
            self.walk_array_specifier(tail.content.ident.content.array_spec.as_ref());
            if let Some(initializer) = &tail.content.initializer {
                self.walk_initializer(initializer);
            }
            if bind_locals {
                self.bind(&tail.content.ident.content.ident);
            }
        }
    }

    fn walk_fully_specified_type(&mut self, ty: &ast::FullySpecifiedType) {
        if let Some(qualifier) = &ty.content.qualifier {
            self.walk_type_qualifier(qualifier);
        }
        self.walk_type_specifier(&ty.content.ty);
    }

    fn walk_type_qualifier(&mut self, qualifier: &ast::TypeQualifier) {
        for specifier in &qualifier.content.qualifiers {
            match &specifier.content {
                ast::TypeQualifierSpecData::Layout(layout) => {
                    for item in &layout.content.ids {
                        if let ast::LayoutQualifierSpecData::Identifier(_, Some(expression)) =
                            &item.content
                        {
                            self.walk_expression(expression);
                        }
                    }
                }
                ast::TypeQualifierSpecData::Storage(storage) => {
                    if let ast::StorageQualifierData::Subroutine(types) = &storage.content {
                        for ty in types {
                            self.walk_type_specifier(ty);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn walk_type_specifier(&mut self, specifier: &ast::TypeSpecifier) {
        if let ast::TypeSpecifierNonArrayData::Struct(struct_specifier) =
            &specifier.content.ty.content
        {
            for field in &struct_specifier.content.fields {
                if let Some(qualifier) = &field.content.qualifier {
                    self.walk_type_qualifier(qualifier);
                }
                self.walk_type_specifier(&field.content.ty);
                for identifier in &field.content.identifiers {
                    self.walk_array_specifier(identifier.content.array_spec.as_ref());
                }
            }
        }
        self.walk_array_specifier(specifier.content.array_specifier.as_ref());
    }

    fn walk_block(&mut self, block: &ast::Block) {
        self.walk_type_qualifier(&block.content.qualifier);
        for field in &block.content.fields {
            if let Some(qualifier) = &field.content.qualifier {
                self.walk_type_qualifier(qualifier);
            }
            self.walk_type_specifier(&field.content.ty);
            for identifier in &field.content.identifiers {
                self.walk_array_specifier(identifier.content.array_spec.as_ref());
            }
        }
        if let Some(identifier) = &block.content.identifier {
            self.walk_array_specifier(identifier.content.array_spec.as_ref());
        }
    }

    fn walk_array_specifier(&mut self, specifier: Option<&ast::ArraySpecifier>) {
        let Some(specifier) = specifier else {
            return;
        };
        for dimension in &specifier.content.dimensions {
            if let ast::ArraySpecifierDimensionData::ExplicitlySized(expression) =
                &dimension.content
            {
                self.walk_expression(expression);
            }
        }
    }

    fn walk_initializer(&mut self, initializer: &ast::Initializer) {
        match &initializer.content {
            ast::InitializerData::Simple(expression) => self.walk_expression(expression),
            ast::InitializerData::List(initializers) => {
                for initializer in initializers {
                    self.walk_initializer(initializer);
                }
            }
        }
    }

    fn walk_compound_statement(&mut self, compound: &ast::CompoundStatement) {
        for statement in &compound.content.statement_list {
            self.walk_statement(statement);
        }
    }

    fn walk_scoped_statement(&mut self, statement: &ast::Statement) {
        self.push_scope();
        self.walk_statement(statement);
        self.pop_scope();
    }

    fn walk_statement(&mut self, statement: &ast::Statement) {
        match &statement.content {
            ast::StatementData::Declaration(declaration) => {
                self.walk_declaration(declaration, true);
            }
            ast::StatementData::Expression(statement) => {
                if let Some(expression) = &statement.content.0 {
                    self.walk_expression(expression);
                }
            }
            ast::StatementData::Selection(selection) => {
                self.walk_expression(&selection.content.cond);
                match &selection.content.rest.content {
                    ast::SelectionRestStatementData::Statement(statement) => {
                        self.walk_scoped_statement(statement);
                    }
                    ast::SelectionRestStatementData::Else(then_statement, else_statement) => {
                        self.walk_scoped_statement(then_statement);
                        self.walk_scoped_statement(else_statement);
                    }
                }
            }
            ast::StatementData::Switch(switch) => {
                self.walk_expression(&switch.content.head);
                self.push_scope();
                for statement in &switch.content.body {
                    self.walk_statement(statement);
                }
                self.pop_scope();
            }
            ast::StatementData::CaseLabel(label) => {
                if let ast::CaseLabelData::Case(expression) = &label.content {
                    self.walk_expression(expression);
                }
            }
            ast::StatementData::Iteration(iteration) => self.walk_iteration(iteration),
            ast::StatementData::Jump(jump) => {
                if let ast::JumpStatementData::Return(Some(expression)) = &jump.content {
                    self.walk_expression(expression);
                }
            }
            ast::StatementData::Compound(compound) => {
                self.push_scope();
                self.walk_compound_statement(compound);
                self.pop_scope();
            }
        }
    }

    fn walk_iteration(&mut self, iteration: &ast::IterationStatement) {
        match &iteration.content {
            ast::IterationStatementData::While(condition, statement) => {
                self.push_scope();
                self.walk_condition(condition);
                self.walk_scoped_statement(statement);
                self.pop_scope();
            }
            ast::IterationStatementData::DoWhile(statement, condition) => {
                self.walk_scoped_statement(statement);
                self.walk_expression(condition);
            }
            ast::IterationStatementData::For(initializer, rest, statement) => {
                self.push_scope();
                match &initializer.content {
                    ast::ForInitStatementData::Expression(expression) => {
                        if let Some(expression) = expression {
                            self.walk_expression(expression);
                        }
                    }
                    ast::ForInitStatementData::Declaration(declaration) => {
                        self.walk_declaration(declaration, true);
                    }
                }
                if let Some(condition) = &rest.content.condition {
                    self.walk_condition(condition);
                }
                if let Some(expression) = &rest.content.post_expr {
                    self.walk_expression(expression);
                }
                self.walk_scoped_statement(statement);
                self.pop_scope();
            }
        }
    }

    fn walk_condition(&mut self, condition: &ast::Condition) {
        match &condition.content {
            ast::ConditionData::Expr(expression) => self.walk_expression(expression),
            ast::ConditionData::Assignment(ty, identifier, initializer) => {
                self.walk_fully_specified_type(ty);
                self.walk_initializer(initializer);
                self.bind(identifier);
            }
        }
    }

    fn walk_expression(&mut self, expression: &ast::Expr) {
        match &expression.content {
            ast::ExprData::Variable(identifier) => self.reference(identifier),
            ast::ExprData::Unary(_, operand)
            | ast::ExprData::PostInc(operand)
            | ast::ExprData::PostDec(operand) => self.walk_expression(operand),
            ast::ExprData::Binary(_, left, right)
            | ast::ExprData::Assignment(left, _, right)
            | ast::ExprData::Bracket(left, right)
            | ast::ExprData::Comma(left, right) => {
                self.walk_expression(left);
                self.walk_expression(right);
            }
            ast::ExprData::Ternary(condition, then_expression, else_expression) => {
                self.walk_expression(condition);
                self.walk_expression(then_expression);
                self.walk_expression(else_expression);
            }
            ast::ExprData::FunCall(identifier, arguments) => {
                match &identifier.content {
                    ast::FunIdentifierData::TypeSpecifier(specifier) => {
                        self.walk_type_specifier(specifier);
                    }
                    ast::FunIdentifierData::Expr(callee) => self.walk_expression(callee),
                }
                for argument in arguments {
                    self.walk_expression(argument);
                }
            }
            ast::ExprData::Dot(base, _) => self.walk_expression(base),
            _ => {}
        }
    }
}

fn collect_used_variables(translation_unit: &ast::TranslationUnit) -> HashSet<String> {
    let mut collector = UsedVariableCollector::default();
    collector.walk_translation_unit(translation_unit);
    collector.names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<UniformDeclaration> {
        parse_glsl_uniforms(source).unwrap()
    }

    fn names(uniforms: &[UniformDeclaration]) -> Vec<&str> {
        uniforms
            .iter()
            .map(|uniform| uniform.name.as_str())
            .collect()
    }

    fn assert_error_contains(source: &str, expected: &str) {
        let error = parse_glsl_uniforms(source).unwrap_err().to_string();
        assert!(
            error.contains(expected),
            "expected error containing {expected:?}, got {error:?}"
        );
    }

    #[test]
    fn keeps_only_used_uniforms_in_declaration_order() {
        let uniforms = parse(
            r#"
                uniform vec4 FIRST;
                uniform vec4 UNUSED;
                uniform float GLOBAL_VALUE;
                float initialized_globally = GLOBAL_VALUE;
                void main() { gl_Position = FIRST; }
            "#,
        );

        assert_eq!(names(&uniforms), ["FIRST", "GLOBAL_VALUE"]);
    }

    #[test]
    fn shadowed_bindings_do_not_activate_uniforms() {
        let uniforms = parse(
            r#"
                uniform float LOCAL_VALUE;
                uniform float PARAMETER_VALUE;
                uniform float FOR_INIT_VALUE;
                uniform float FOR_CONDITION_VALUE;
                uniform float WHILE_CONDITION_VALUE;

                float parameter_only(float PARAMETER_VALUE) {
                    return PARAMETER_VALUE;
                }

                void main() {
                    float LOCAL_VALUE = 0.0;
                    LOCAL_VALUE;
                    for (float FOR_INIT_VALUE = 0.0; FOR_INIT_VALUE < 1.0; ++FOR_INIT_VALUE) {
                        FOR_INIT_VALUE;
                    }
                    for (; bool FOR_CONDITION_VALUE = false; ) {
                        FOR_CONDITION_VALUE;
                        break;
                    }
                    while (bool WHILE_CONDITION_VALUE = false) {
                        WHILE_CONDITION_VALUE;
                        break;
                    }
                    gl_Position = vec4(0.0);
                }
            "#,
        );

        assert!(uniforms.is_empty());
    }

    #[test]
    fn scope_exit_restores_uniform_visibility() {
        let uniforms = parse(
            r#"
                uniform float BLOCK_VALUE;
                uniform float IF_VALUE;
                uniform float FOR_VALUE;
                uniform float WHILE_VALUE;
                uniform float PARAMETER_VALUE;

                float helper(float PARAMETER_VALUE) {
                    return PARAMETER_VALUE;
                }

                void main() {
                    { float BLOCK_VALUE = 0.0; BLOCK_VALUE; }
                    if (true) { float IF_VALUE = 0.0; IF_VALUE; }
                    for (; bool FOR_VALUE = false; ) { FOR_VALUE; break; }
                    while (bool WHILE_VALUE = false) { WHILE_VALUE; break; }
                    gl_Position = vec4(
                        BLOCK_VALUE + IF_VALUE + FOR_VALUE + WHILE_VALUE + PARAMETER_VALUE
                    );
                }
            "#,
        );

        assert_eq!(
            names(&uniforms),
            [
                "BLOCK_VALUE",
                "IF_VALUE",
                "FOR_VALUE",
                "WHILE_VALUE",
                "PARAMETER_VALUE"
            ]
        );
    }

    #[test]
    fn walks_uniform_references_in_all_statement_positions() {
        let uniforms = parse(
            r#"
                uniform float GLOBAL_INITIALIZER;
                uniform float DECLARATION_INITIALIZER;
                uniform float NESTED_BLOCK;
                uniform float IF_CONDITION;
                uniform float IF_BODY;
                uniform float ELSE_BODY;
                uniform float SWITCH_HEAD;
                uniform float SWITCH_BODY;
                uniform float FOR_INITIALIZER;
                uniform float FOR_CONDITION;
                uniform float FOR_POST;
                uniform float FOR_BODY;
                uniform float WHILE_CONDITION;
                uniform float WHILE_BODY;
                uniform float DO_BODY;
                uniform float DO_CONDITION;
                uniform float CALL_ARGUMENT;
                uniform float RETURN_VALUE;

                float global_value = GLOBAL_INITIALIZER;
                float consume(float value) { return value; }
                float exercise() {
                    float value = DECLARATION_INITIALIZER;
                    { value += NESTED_BLOCK; }
                    if (IF_CONDITION > 0.0) {
                        value += IF_BODY;
                    } else {
                        value += ELSE_BODY;
                    }
                    switch (int(SWITCH_HEAD)) {
                        case 0: value += SWITCH_BODY; break;
                        default: break;
                    }
                    for (
                        float index = FOR_INITIALIZER;
                        index < FOR_CONDITION;
                        index += FOR_POST
                    ) {
                        value += FOR_BODY;
                    }
                    while (WHILE_CONDITION > 0.0) {
                        value += WHILE_BODY;
                        break;
                    }
                    do {
                        value += DO_BODY;
                    } while (DO_CONDITION > 0.0);
                    value += consume(CALL_ARGUMENT);
                    return value + RETURN_VALUE;
                }
                void main() {
                    gl_Position = vec4(exercise() + global_value);
                }
            "#,
        );

        assert_eq!(
            names(&uniforms),
            [
                "GLOBAL_INITIALIZER",
                "DECLARATION_INITIALIZER",
                "NESTED_BLOCK",
                "IF_CONDITION",
                "IF_BODY",
                "ELSE_BODY",
                "SWITCH_HEAD",
                "SWITCH_BODY",
                "FOR_INITIALIZER",
                "FOR_CONDITION",
                "FOR_POST",
                "FOR_BODY",
                "WHILE_CONDITION",
                "WHILE_BODY",
                "DO_BODY",
                "DO_CONDITION",
                "CALL_ARGUMENT",
                "RETURN_VALUE"
            ]
        );
    }

    #[test]
    fn a_binding_initializer_can_still_reference_the_shadowed_uniform() {
        let uniforms = parse(
            r#"
                uniform float VALUE;
                void main() {
                    float VALUE = VALUE;
                    gl_Position = vec4(0.0);
                }
            "#,
        );

        assert_eq!(names(&uniforms), ["VALUE"]);
    }

    #[test]
    fn uncalled_functions_are_conservatively_scanned_without_a_call_graph() {
        let uniforms = parse(
            r#"
                uniform float MAYBE_ACTIVE;
                float never_called() { return MAYBE_ACTIVE; }
                void main() { gl_Position = vec4(0.0); }
            "#,
        );

        assert_eq!(names(&uniforms), ["MAYBE_ACTIVE"]);
    }

    #[test]
    fn collects_all_declarators_and_their_array_lengths() {
        let uniforms = parse(
            r#"
                uniform vec4 FIRST, SECOND[2], THIRD;
                void main() { gl_Position = FIRST + SECOND[0] + THIRD; }
            "#,
        );

        assert_eq!(names(&uniforms), ["FIRST", "SECOND", "THIRD"]);
        assert_eq!((uniforms[0].array_count, uniforms[0].is_array), (1, false));
        assert_eq!((uniforms[1].array_count, uniforms[1].is_array), (2, true));
        assert_eq!((uniforms[2].array_count, uniforms[2].is_array), (1, false));
    }

    #[test]
    fn finds_uniform_storage_qualifier_in_any_position() {
        let uniforms = parse(
            r#"
                highp uniform vec4 PARAMS;
                void main() { gl_Position = PARAMS; }
            "#,
        );

        assert_eq!(names(&uniforms), ["PARAMS"]);
    }

    #[test]
    fn expands_macro_array_length() {
        let uniforms = parse(
            r#"
                #define SAMPLE_COUNT 6
                uniform sampler2D SAMPLER[SAMPLE_COUNT];
                void main() { gl_FragColor = texture2D(SAMPLER[5], vec2(0.0)); }
            "#,
        );

        assert_eq!((uniforms[0].array_count, uniforms[0].is_array), (6, true));
    }

    #[test]
    fn accepts_positive_signed_and_unsigned_array_literals() {
        let uniforms = parse(
            r#"
                uniform vec4 SIGNED_ARRAY[2];
                uniform vec4 UNSIGNED_ARRAY[3u];
                void main() {
                    gl_Position = SIGNED_ARRAY[0] + UNSIGNED_ARRAY[0];
                }
            "#,
        );

        assert_eq!(uniforms[0].array_count, 2);
        assert_eq!(uniforms[1].array_count, 3);
    }

    #[test]
    fn parses_scalar_vector_matrix_and_integer_defaults() {
        let uniforms = parse(
            r#"
                uniform float NEGATIVE = -1.5;
                uniform float NEGATIVE_ZERO = -0.0;
                uniform vec3 SPLAT = vec3(0.25);
                uniform vec4 ORDERED = vec4(1.0, -2.0, 3u, +4.5);
                uniform mat2 MATRIX = mat2(1.0, 2.0, 3.0, 4.0);
                uniform mat3 DIAGONAL = mat3(2.0);
                uniform int INTEGER = -8;
                uniform int LARGE_INTEGER = 16777217;
                void main() {
                    NEGATIVE; NEGATIVE_ZERO; SPLAT; ORDERED;
                    MATRIX; DIAGONAL; INTEGER; LARGE_INTEGER;
                    gl_Position = vec4(0.0);
                }
            "#,
        );

        let bits = |values: &[f32]| {
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(uniforms[0].default_data, Some(bits(&[-1.5])));
        assert_eq!(uniforms[1].default_data, Some(bits(&[-0.0])));
        assert_eq!(uniforms[2].default_data, Some(bits(&[0.25, 0.25, 0.25])));
        assert_eq!(uniforms[3].default_data, Some(bits(&[1.0, -2.0, 3.0, 4.5])));
        assert_eq!(uniforms[4].default_data, Some(bits(&[1.0, 2.0, 3.0, 4.0])));
        assert_eq!(
            uniforms[5].default_data,
            Some(bits(&[2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0]))
        );
        assert_eq!(uniforms[6].default_data, Some(bits(&[-8.0])));
        assert_eq!(uniforms[7].default_data, Some(bits(&[16_777_216.0])));
    }

    #[test]
    fn preserves_supported_nested_and_alias_constructors() {
        let uniforms = parse(
            r#"
                uniform vec4 NESTED_VECTOR = vec4(vec2(1.0, 2.0), vec2(3.0, 4.0));
                uniform mat2 MATRIX_ALIAS = mat2x2(1.0, 2.0, 3.0, 4.0);
                uniform mat2x3 NESTED_MATRIX = mat2x3(mat2x3(1.0, 2.0, 3.0, 4.0, 5.0, 6.0));
                void main() {
                    NESTED_VECTOR; MATRIX_ALIAS; NESTED_MATRIX;
                    gl_Position = vec4(0.0);
                }
            "#,
        );

        let bits = |values: &[f32]| {
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(uniforms[0].default_data, Some(bits(&[1.0, 2.0, 3.0, 4.0])));
        assert_eq!(uniforms[1].default_data, Some(bits(&[1.0, 2.0, 3.0, 4.0])));
        assert_eq!(
            uniforms[2].default_data,
            Some(bits(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]))
        );
    }

    #[test]
    fn rejects_unary_signs_on_unsigned_literals() {
        assert_error_contains(
            "uniform float VALUE = -1u; void main() { gl_Position = vec4(VALUE); }",
            "无符号整数字面量",
        );
        assert_error_contains(
            "uniform float VALUE = +1u; void main() { gl_Position = vec4(VALUE); }",
            "无符号整数字面量",
        );
    }

    #[test]
    fn rejects_mismatched_top_level_and_nested_matrix_constructors() {
        assert_error_contains(
            "uniform vec4 VALUE = mat2(1.0, 2.0, 3.0, 4.0); void main() { gl_Position = VALUE; }",
            "与声明类型",
        );
        assert_error_contains(
            "uniform mat2 VALUE = vec4(1.0, 2.0, 3.0, 4.0); void main() { VALUE; gl_Position = vec4(0.0); }",
            "与声明类型",
        );
        assert_error_contains(
            "uniform mat2x3 VALUE = mat2x3(mat3x2(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)); void main() { VALUE; gl_Position = vec4(0.0); }",
            "异型矩阵构造器转换",
        );
    }

    #[test]
    fn each_declarator_keeps_its_own_default_and_arrays_have_no_block() {
        let uniforms = parse(
            r#"
                uniform float FIRST = 1.0, SECOND = -2.0;
                uniform vec2 VALUES[2] = vec2(9.0);
                void main() {
                    gl_Position = vec4(FIRST + SECOND + VALUES[0].x);
                }
            "#,
        );

        assert_eq!(uniforms[0].default_data, Some(vec![1.0f32.to_bits()]));
        assert_eq!(uniforms[1].default_data, Some(vec![(-2.0f32).to_bits()]));
        assert_eq!(uniforms[2].default_data, None);
    }

    #[test]
    fn rejects_defaults_that_cannot_be_encoded_deterministically() {
        assert_error_contains(
            "uniform float VALUE = 1.0 + 2.0; void main() { gl_Position = vec4(VALUE); }",
            "只支持数值字面量",
        );
        assert_error_contains(
            "uniform vec3 VALUE = vec3(1.0, 2.0); void main() { gl_Position = vec4(VALUE, 1.0); }",
            "需要 1 个或 3 个",
        );
        assert_error_contains(
            "uniform sampler2D TEX = sampler2D(0); void main() { gl_FragColor = texture2D(TEX, vec2(0.0)); }",
            "不能包含初始化值",
        );
        assert_error_contains(
            "uniform sampler2D TEX[1] = sampler2D(0); void main() { gl_FragColor = texture2D(TEX[0], vec2(0.0)); }",
            "不能包含初始化值",
        );
    }

    #[test]
    fn rejects_invalid_array_declarations() {
        assert_error_contains(
            "uniform vec4 U[]; void main() { gl_Position = U[0]; }",
            "未指定长度",
        );
        assert_error_contains(
            "uniform vec4 U[0]; void main() { gl_Position = U[0]; }",
            "必须为正整数",
        );
        assert_error_contains(
            "uniform vec4 U[-1]; void main() { gl_Position = U[0]; }",
            "正整数字面量",
        );
        assert_error_contains(
            "uniform vec4 U[2][3]; void main() { gl_Position = U[0][0]; }",
            "KSH 只支持一维数组",
        );
        assert_error_contains(
            "const int N = 2; uniform vec4 U[N]; void main() { gl_Position = U[0]; }",
            "正整数字面量",
        );
    }

    #[test]
    fn visits_for_declaration_and_condition_assignment_initializers() {
        let uniforms = parse(
            r#"
                uniform float FOR_VALUE;
                uniform float WHILE_VALUE;
                void main() {
                    for (float value = FOR_VALUE; value < 1.0; ++value) { }
                    while (bool keep_going = WHILE_VALUE > 0.0) { break; }
                    gl_Position = vec4(0.0);
                }
            "#,
        );

        assert_eq!(names(&uniforms), ["FOR_VALUE", "WHILE_VALUE"]);
    }

    #[test]
    fn walker_traverses_function_callees_and_arguments() {
        let translation_unit = ast::TranslationUnit::parse::<glsl_lang::parse::DefaultLexer>(
            "float shade(float value) { return value; } void main() { shade(ARGUMENT); }",
        )
        .unwrap();
        let used = collect_used_variables(&translation_unit);

        assert!(used.contains("shade"));
        assert!(used.contains("ARGUMENT"));
    }

    #[test]
    fn rejects_duplicate_uniform_declarations() {
        assert_error_contains(
            r#"
                uniform vec4 PARAMS;
                uniform vec4 PARAMS;
                void main() { gl_Position = PARAMS; }
            "#,
            "重复声明",
        );
    }

    #[test]
    fn rejects_unsupported_uniform_type_without_panicking() {
        assert_error_contains(
            "uniform bool ENABLED; void main() { gl_Position = vec4(ENABLED); }",
            "不受 KSH 支持",
        );
    }
}
