const rust = require('tree-sitter-rust/grammar');
module.exports = grammar(rust, {
  name: 'bare_rust',
  rules: {
    struct_item: $ => seq('struct', field('name', $._type_identifier),
      choice(field('body', $.field_declaration_list), seq(field('body', $.ordered_field_declaration_list), ';'), ';')),
    field_declaration: $ => field('name', $._field_identifier),
    ordered_field_declaration_list: $ => seq('(', sepBy(',', '_'), optional(','), ')'),
    function_item: $ => seq(optional($.function_modifiers), 'fn', field('name', choice($.identifier, $.metavariable)),
      field('parameters', $.parameters), field('body', $.block)),
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
