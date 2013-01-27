// http://dev.w3.org/csswg/css3-syntax/#tree-construction
//
// The input to the tree construction stage is a sequence of tokens
// from the tokenization stage.
// The output is a tree of items with a stylesheet at the root
// and all other nodes being at-rules, style rules, or declarations.


use tokens;


#[deriving_eq]
enum Primitive {
    // Preserved tokens. Same as in the tokenizer.
    Ident(~str),
    AtKeyword(~str),
    Hash(~str),
    String(~str),
    URL(~str),
    Delim(char),
    Number(NumericValue, ~str),  // value, representation
    Percentage(NumericValue, ~str),  // value, representation
    Dimension(NumericValue, ~str, ~str),  // value, representation, unit
    UnicodeRange(char, char),  // start, end
    EmptyUnicodeRange,
    WhiteSpace,
    Comment,
    CDO,  // <!--
    CDC,  // -->
    Colon,  // :
    Semicolon,  // ;
    CloseBraket, // ]
    CloseParen, // )
    CloseBrace, // }

    // Function
    Function(~str, ~[~[Primitive]]),  // name, arguments

    // Simple block
    BraketBlock(~[Primitive]),  // […]
    ParenBlock(~[Primitive]),  // (…)
    BraceBlock(~[Primitive]),  // {…}
}


struct Declaration {
    name: ~str,
    value: ~[Primitive],
    important: bool,
}

struct StyleRule {
    selector: ~[Primitive],
    value: ~[DeclarationListItem],
}

struct AtRule {
    name: ~str,
    selector: ~[Primitive],
    value: AtRuleValue,
}


enum AtRuleValue {
    EmptyAtRule,  // @foo…;
    DeclarationFilled(~[DeclarationListItem]),
    RuleFilled(~[RuleListItem]),
}

enum DeclarationListItem {
    Declaration(Declaration),
    // A better idea for a name that means "at-rule" but is not "AtRule"?
    Decl_AtRule(AtRule),
}

enum RuleListItem {
    StyleRule(StyleRule),
    AtRule(AtRule),
}


pub struct Parser {
    priv tokenizer: ~tokens::Tokenizer,
    priv quirks_mode: bool,
    priv mut current_token: Option<tokens::Token>,
    priv mut errors: ~[~str],
}


fn make_parser(tokenizer: ~tokens::Tokenizer, quirks_mode: bool) -> ~Parser {
    ~Parser {
        tokenizer: tokenizer,
        quirks_mode: quirks_mode,
        current_token: None,
        errors: ~[],
    }
}


fn parser_from_string(input: &str, transform_function_whitespace: bool,
                      quirks_mode: bool) -> ~Parser {
    make_parser(tokens::make_tokenizer(input, transform_function_whitespace),
                quirks_mode)
}


// Consume the whole input and return a list of primitives.
// Could be used for parsing eg. a stand-alone media query.
// This is similar to consume_simple_block(), but there is no ending token.
fn consume_primitive_list(parser: &Parser) -> ~[Primitive] {
    let mut value: ~[Primitive] = ~[];
    loop {
        match consume_next_token(parser) {
            tokens::Comment => (),
            tokens::EOF => break,
            token => {
                reconsume_token(parser, token);
                value.push(consume_primitive(parser))
            }
        }
    }
    value
}


//  ***********  End of public API  ***********


fn consume_next_token(parser: &Parser) -> tokens::Token {
    let mut current_token = None;
    current_token <-> parser.current_token;
    match current_token {
        Some(token) => token,
        None => match parser.tokenizer.next_token() {
            tokens::TokenResult {token: token, error: err} => {
                match err {
                    // TODO more contextual error handling?
                    Some(err) => parser.errors.push(err),
                    None => ()
                }
                token
            }
        }
    }
}


// Fail if the is already a "current token".
// The easiest way to ensure this does not fail it to call it only once
// just after calling consume_next_token()
fn reconsume_token(parser: &Parser, token: tokens::Token) {
    assert parser.current_token.is_none();
    parser.current_token = Some(token)
}


// Convert a token from the tokenizer to a primitive.
// Fails if it is not a preserved token. XXX do something else.
fn preserved_token_to_primitive(token: tokens::Token) -> Primitive {
    match token {
        tokens::Ident(string) => Ident(string),
        tokens::AtKeyword(string) => AtKeyword(string),
        tokens::Hash(string) => Hash(string),
        tokens::String(string) => String(string),
        tokens::URL(string) => URL(string),
        tokens::Delim(ch) => Delim(ch),
        tokens::Number(value, repr) => Number(value, repr),
        tokens::Percentage(value, repr) => Percentage(value, repr),
        tokens::Dimension(value, repr, unit) => Dimension(value, repr, unit),
        tokens::UnicodeRange(start, end) => UnicodeRange(start, end),
        tokens::EmptyUnicodeRange => EmptyUnicodeRange,
        tokens::WhiteSpace => WhiteSpace,
        tokens::Colon => Colon,
        tokens::Semicolon => Semicolon,
        _ => fail,   // XXX
        // These are special-cased in consume_primitive()
//        tokens::Function(string) => fail,
//        tokens::OpenBraket => fail,
//        tokens::OpenParen => fail,
//        tokens::OpenBrace => fail,

        // These still need to be dealt with somehow.
//        tokens::BadString => fail,
//        tokens::BadURL => fail,
//        tokens::Comment => fail,
//        tokens::CDO => fail,
//        tokens::CDC => fail,
//        tokens::CloseBraket => fail,
//        tokens::CloseParen => fail,
//        tokens::CloseBrace => fail,
//        tokens::EOF => fail,
    }
}


// 3.5.15. Consume a primitive
fn consume_primitive(parser: &Parser) -> Primitive {
    match consume_next_token(parser) {
        tokens::OpenBraket =>
            BraketBlock(consume_simple_block(parser, tokens::CloseBraket)),
        tokens::OpenParen =>
            ParenBlock(consume_simple_block(parser, tokens::CloseParen)),
        tokens::OpenBrace =>
            BraceBlock(consume_simple_block(parser, tokens::CloseBrace)),
        tokens::Function(string) => consume_function(parser, string),
        token => preserved_token_to_primitive(token),
    }
}


// 3.5.18. Consume a simple block  (kind of)
fn consume_simple_block(parser: &Parser, ending_token: tokens::Token)
        -> ~[Primitive] {
    let mut value: ~[Primitive] = ~[];
    loop {
        match consume_next_token(parser) {
            tokens::Comment => (),
            tokens::EOF => break,
            token => {
                if token == ending_token { break }
                else {
                    reconsume_token(parser, token);
                    value.push(consume_primitive(parser))
                }
            }
        }
    }
    value
}


// 3.5.19. Consume a function
fn consume_function(parser: &Parser, name: ~str)
        -> Primitive {
    let mut current_argument: ~[Primitive] = ~[];
    let mut arguments: ~[~[Primitive]] = ~[];
    loop {
        match consume_next_token(parser) {
            tokens::Comment => (),
            tokens::EOF | tokens::CloseParen => break,
            tokens::Delim(',') => {
                arguments.push(current_argument);
                current_argument = ~[];
            },
            tokens::Number(value, repr) => current_argument.push(
                // XXX case-sensitive or ASCII case-insensitive? See
            // http://lists.w3.org/Archives/Public/www-style/2013Jan/0480.html
                if parser.quirks_mode && ascii_lower(name) == ~"rect" {
                    // 3.5.17. Consume a primitive
                    // with the unitless length quirk
                    Dimension(value, repr, ~"px")
                } else {
                    Number(value, repr)
                }
            ),
            token => {
                reconsume_token(parser, token);
                current_argument.push(consume_primitive(parser))
            }
        }
    }
    arguments.push(current_argument);
    Function(name, arguments)
}


#[test]
fn test_primitives() {
    fn assert_primitives(input: &str, expected_primitives: &[Primitive],
                         expected_errors: &[~str]) {
        assert_primitives_flags(
            input, expected_primitives, expected_errors, false)
    }
    fn assert_primitives_flags(input: &str, expected_primitives: &[Primitive],
                               expected_errors: &[~str], quirks_mode: bool) {
        let parser = parser_from_string(input, false, quirks_mode);
        let primitives: &[Primitive] = consume_primitive_list(parser);
        let errors: &[~str] = parser.errors;
        if primitives != expected_primitives {
            fail fmt!("%?\n!=\n%?", primitives, expected_primitives);
        }
        if errors != expected_errors {
            fail fmt!("%?\n!=\n%?", errors, expected_errors);
        }
    }

    assert_primitives("", [], []);
    assert_primitives("42 foo([aa ()b], -){\n  }", [
        Number(Integer(42), ~"42"), WhiteSpace,
        Function(~"foo", ~[
            ~[BraketBlock(~[
                Ident(~"aa"), WhiteSpace, ParenBlock(~[]), Ident(~"b")])],
            ~[WhiteSpace, Delim('-')]]),
        BraceBlock(~[
            WhiteSpace])], [])
}
