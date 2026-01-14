/**
 * @file A structured format for logging nutrition balance data
 * @author Josh Fayer <dev@fayer.me>
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

export default grammar({
  name: "nutrition",

  rules: {
    // TODO: add the actual grammar rules
      ///////////////////////
      // Top-level source
      ///////////////////////
      // allow top-level standalone comments as well as declarations
      source_file: $ => repeat(choice($.comment, $._declaration)),

      // top-level declaration (comments are handled as standalone nodes in source_file)
      _declaration: $ => choice(
        $.unit_decl,
        $.property_decl,
        $.ingredient_decl,
        $.food_decl,
        $.recipe_decl,
        $.exercise_decl,
        $.day_decl,
        $.ate_entry,
        $.exercised_entry
      ),

      ///////////////////////
      // Lexical
      ///////////////////////
      comment: $ => token(seq('//', /.*/)),

      string: $ => token(seq('"', repeat(choice(/[^"\\]/, seq('\\', /./))), '"')),

      number: $ => token(choice(
        /[0-9]+\.[0-9]+/,
        /[0-9]+/,
        /\.[0-9]+/
      )),

      identifier: $ => /[A-Za-z_][A-Za-z0-9_\-]*/,

      unit_token: $ => choice($.string, $.identifier),

      ///////////////////////
      // Unit declarations
      ///////////////////////
      unit_decl: $ => seq(
        '@unit',
        repeat1($.unit_name),
        optional(repeat1(seq('=', $.number, repeat1($.unit_name))))
      ),

      unit_name: $ => choice($.string, $.identifier),

      ///////////////////////
      // Property declarations
      ///////////////////////
      property_decl: $ => seq(
        '@property',
        repeat1($.string),
        choice('Int', 'Float', 'Bool'),
        optional($.unit_token)
      ),

      ///////////////////////
      // Ingredients & Food
      ///////////////////////
      ingredient_decl: $ => seq(
        '@ingredient',
        repeat1($.paren_quantity),
        repeat1($.string),
        $.block
      ),

      food_decl: $ => seq(
        '@food',
        repeat1($.paren_quantity),
        repeat1($.string),
        $.block
      ),

      paren_quantity: $ => seq('(', $.number, optional($.unit_token), ')'),

      block_separator: $ => choice(',', '\n'),

      // an item inside a block can have an optional end-of-line comment
      block_item: $ => seq(choice($.property_assignment, $.ate_entry, $.exercised_entry), optional($.comment)),

      block: $ => seq(
        '{',
        repeat(seq(choice($.block_item, $.comment), $.block_separator)),
        '}'
      ),
      
      property_assignment: $ => seq(
        $.identifier,
        ':',
        $.value
      ),
      
      value: $ => choice($.number_with_unit, $.bool, $.string),

      number_with_unit: $ => seq($.number, optional($.unit_token)),

      bool: $ => choice('true', 'false', 'True', 'False'),

      ///////////////////////
      // Recipes
      ///////////////////////
      // allow an ingredient line to have an optional trailing comment
      recipe_item: $ => seq($.recipe_ingredient_line, optional($.comment)),

      recipe_decl: $ => seq(
        '@recipe',
        repeat1($.paren_quantity),
        repeat1($.string),
        '{',
          optional(seq(
            choice($.recipe_item, $.comment),
            repeat(seq($.block_separator, choice($.recipe_item, $.comment))),
            optional($.block_separator)
          )),
        '}'
      ),
      
      recipe_ingredient_line: $ => seq($.string, $.paren_quantity),
      
      ///////////////////////
      // Exercise
      ///////////////////////
      exercise_decl: $ => seq(
        '@exercise',
        $.paren_quantity,
        repeat1($.string,),
        $.block
      ),

      ///////////////////////
      // Day / diary
      ///////////////////////
      // day entries can have trailing comments and use block_separator (newline/comma) between items
      day_item: $ => seq(choice($.meal_label, $.ate_entry, $.exercised_entry), optional($.comment)),

      day_decl: $ => seq(
        '@day',
        $.string,
        '{',
          optional(seq(
            choice($.day_item, $.comment),
            repeat(seq($.block_separator, choice($.day_item, $.comment))),
            optional($.block_separator)
          )),
        '}'
      ),
      
      meal_label: $ => token(seq('[', /[^\]\n]+/, ']')),

      ate_entry: $ => seq('@ate', $.string, optional($.paren_quantity)),

      exercised_entry: $ => seq('@exercised', $.string, optional($.paren_quantity)),

      ///////////////////////
      // Extras
      ///////////////////////
      _newline: $ => /\n/,
  }
});
