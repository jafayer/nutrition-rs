use logos::Logos;
use nutrition_rs::lexer::lexer::Token;

#[test]
fn test_keywords() {
    let mut lexer = Token::lexer("@unit @property @ingredient @food @recipe @exercise @day @ate @exercised");
    
    assert_eq!(lexer.next(), Some(Ok(Token::AtUnit)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtProperty)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtIngredient)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtFood)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtRecipe)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtExercise)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtDay)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtAte)));
    assert_eq!(lexer.next(), Some(Ok(Token::AtExercised)));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_punctuation() {
    let mut lexer = Token::lexer("{ } ( ) : , =");
    
    assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
    assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
    assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
    assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
    assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
    assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
    assert_eq!(lexer.next(), Some(Ok(Token::Equals)));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_strings() {
    let mut lexer = Token::lexer(r#""hello" "world" "test string""#);
    
    assert_eq!(lexer.next(), Some(Ok(Token::String("hello".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::String("world".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::String("test string".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_strings_with_escapes() {
    let mut lexer = Token::lexer(r#""hello\"world" "tab\there" "newline\n""#);
    
    assert_eq!(lexer.next(), Some(Ok(Token::String("hello\"world".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::String("tab\there".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::String("newline\n".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_integers() {
    let mut lexer = Token::lexer("0 42 999 1000");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Number(0.0))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(42.0))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(999.0))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(1000.0))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_floats() {
    let mut lexer = Token::lexer("3.14 0.5 100.99 .5");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Number(3.14))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(0.5))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(100.99))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(0.5))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_booleans() {
    let mut lexer = Token::lexer("true false True False");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Bool(true))));
    assert_eq!(lexer.next(), Some(Ok(Token::Bool(false))));
    assert_eq!(lexer.next(), Some(Ok(Token::Bool(true))));
    assert_eq!(lexer.next(), Some(Ok(Token::Bool(false))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_identifiers() {
    let mut lexer = Token::lexer("apple banana _private name-with-dash Test123");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("banana".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("_private".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("name-with-dash".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Test123".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_meal_labels() {
    let mut lexer = Token::lexer("[Breakfast] [Lunch] [Morning Snack]");
    
    assert_eq!(lexer.next(), Some(Ok(Token::MealLabel("[Breakfast]".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::MealLabel("[Lunch]".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::MealLabel("[Morning Snack]".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_comments() {
    let mut lexer = Token::lexer("apple // comment\nbanana");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Comment("// comment".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("banana".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_newlines() {
    let mut lexer = Token::lexer("apple\nbanana\ncherry");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("banana".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("cherry".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_whitespace_is_skipped() {
    let mut lexer = Token::lexer("apple   banana\t\tcherry  \r  orange");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("banana".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("cherry".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("orange".to_string()))));
    assert_eq!(lexer.next(), None);
}


#[test]
fn test_ingredient_with_properties() {
    let input = "@ingredient apple (100.5 g)";
    let mut lexer = Token::lexer(input);
    
    assert_eq!(lexer.next(), Some(Ok(Token::AtIngredient)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(100.5))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("g".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_meal_with_items() {
    let input = "[Breakfast]\n\"apple\"\n\"banana\"(2)\n\"orange\"";
    let mut lexer = Token::lexer(input);
    
    assert_eq!(lexer.next(), Some(Ok(Token::MealLabel("[Breakfast]".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::String("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::String("banana".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(2.0))));
    assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::String("orange".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_empty_input() {
    let mut lexer = Token::lexer("");
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_only_whitespace() {
    let mut lexer = Token::lexer("   \t  \r  ");
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_only_newlines() {
    let mut lexer = Token::lexer("\n\n\n");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_identifier_with_numbers() {
    let mut lexer = Token::lexer("var1 test_var _test123 var-name");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("var1".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("test_var".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("_test123".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("var-name".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_numbers_with_leading_zero() {
    let mut lexer = Token::lexer("0.1 0.99 0123 01.5");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Number(0.1))));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(0.99))));
    // 0123 is parsed as 123
    assert!(matches!(lexer.next(), Some(Ok(Token::Number(n))) if (n - 123.0).abs() < 0.001));
    assert_eq!(lexer.next(), Some(Ok(Token::Number(1.5))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_meal_label_with_special_chars() {
    let mut lexer = Token::lexer("[Before Workout] [Post-Workout Meal]");
    
    assert_eq!(lexer.next(), Some(Ok(Token::MealLabel("[Before Workout]".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::MealLabel("[Post-Workout Meal]".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_string_empty() {
    let mut lexer = Token::lexer(r#""""#);
    
    assert_eq!(lexer.next(), Some(Ok(Token::String("".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_comment_with_special_chars() {
    let mut lexer = Token::lexer("apple // this is a comment with !@#$%^&*() chars\nbanana");
    
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("apple".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Comment("// this is a comment with !@#$%^&*() chars".to_string()))));
    assert_eq!(lexer.next(), Some(Ok(Token::Newline)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("banana".to_string()))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_all_at_keywords() {
    let keywords = vec![
        ("@unit", Token::AtUnit),
        ("@property", Token::AtProperty),
        ("@ingredient", Token::AtIngredient),
        ("@food", Token::AtFood),
        ("@recipe", Token::AtRecipe),
        ("@exercise", Token::AtExercise),
        ("@day", Token::AtDay),
        ("@ate", Token::AtAte),
        ("@exercised", Token::AtExercised),
    ];
    
    for (keyword, expected) in keywords {
        let mut lexer = Token::lexer(keyword);
        assert_eq!(lexer.next(), Some(Ok(expected)), "Failed for keyword: {}", keyword);
        assert_eq!(lexer.next(), None);
    }
}

// write a test that loads examples/test.nutrition and lexes it completely without errors
#[test]
fn test_lex_example_file() {
    use std::fs;
    use std::path::Path;
    let path = Path::new("examples/test.nutrition");
    let content = fs::read_to_string(path).expect("Failed to read example file");
    let mut lexer = Token::lexer(&content);
    while let Some(token) = lexer.next() {
        assert!(token.is_ok(), "Lexing error: {:?}", token);
    }
}