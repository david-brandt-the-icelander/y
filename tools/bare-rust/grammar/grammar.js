// Bare Rust: tree-sitter-rust with every constraint production made empty (proposal/bare-rust.md, 3.1).
// Names stay; types, lifetimes, `&`/`&mut`/`mut`, visibility, generics, bounds and `where` go to the constraint file.
const rust = require('tree-sitter-rust/grammar');
module.exports = grammar(rust, {
  name: 'bare_rust',
  rules: {
    // Associated types are type aliases here: both are `type Name;`.
    _declaration_statement: $ => choice(
      $.const_item, $.macro_invocation, $.macro_definition, $.empty_statement, $.attribute_item,
      $.inner_attribute_item, $.mod_item, $.foreign_mod_item, $.struct_item, $.union_item, $.enum_item,
      $.type_item, $.function_item, $.function_signature_item, $.impl_item, $.trait_item, $.let_declaration,
      $.use_declaration, $.extern_crate_declaration, $.static_item),

    mod_item: $ => seq('mod', field('name', $.identifier), choice(';', field('body', $.declaration_list))),

    struct_item: $ => seq('struct', field('name', $._type_identifier),
      choice(field('body', $.field_declaration_list), seq(field('body', $.ordered_field_declaration_list), ';'), ';')),
    union_item: $ => seq('union', field('name', $._type_identifier), field('body', $.field_declaration_list)),
    enum_item: $ => seq('enum', field('name', $._type_identifier), field('body', $.enum_variant_list)),
    enum_variant: $ => seq(field('name', $.identifier),
      field('body', optional(choice($.field_declaration_list, $.ordered_field_declaration_list))),
      optional(seq('=', field('value', $._expression)))),
    field_declaration: $ => field('name', $._field_identifier),
    ordered_field_declaration_list: $ => seq('(', sepBy(',', seq(repeat($.attribute_item), '_')), optional(','), ')'),

    trait_item: $ => seq(optional('unsafe'), 'trait', field('name', $._type_identifier), field('body', $.declaration_list)),
    impl_item: $ => seq(optional('unsafe'), 'impl',
      optional(seq(optional('!'), field('trait', choice($._type_identifier, $.scoped_type_identifier)), 'for')),
      field('type', choice($._type_identifier, $.scoped_type_identifier)),
      choice(field('body', $.declaration_list), ';')),

    const_item: $ => seq('const', field('name', choice($.identifier, '_')),
      optional(seq('=', field('value', $._expression))), ';'),
    static_item: $ => seq('static', optional('ref'), optional($.mutable_specifier), field('name', $.identifier),
      optional(seq('=', field('value', $._expression))), ';'),
    // A type alias moves whole into the file.
    type_item: $ => seq('type', field('name', $._type_identifier), ';'),

    function_item: $ => seq(optional($.function_modifiers), 'fn', field('name', choice($.identifier, $.metavariable)),
      field('parameters', $.parameters), field('body', $.block)),
    function_signature_item: $ => seq(optional($.function_modifiers), 'fn', field('name', choice($.identifier, $.metavariable)),
      field('parameters', $.parameters), ';'),
    parameters: $ => seq('(', sepBy(',', seq(optional($.attribute_item), choice($.parameter, $.self_parameter, '_'))), optional(','), ')'),
    self_parameter: $ => $.self,
    parameter: $ => field('pattern', $._pattern),
    closure_parameters: $ => seq('|', sepBy(',', $.parameter), '|'),
    closure_expression: $ => prec(-1, seq(optional('static'), optional('async'), optional('move'),
      field('parameters', $.closure_parameters), field('body', choice($._expression, '_')))),
    let_declaration: $ => seq('let', field('pattern', $._pattern),
      optional(seq('=', field('value', $._expression))), optional(seq('else', field('alternative', $.block))), ';'),
  },
});
function sepBy(sep, rule) { return optional(seq(rule, repeat(seq(sep, rule)))); }
