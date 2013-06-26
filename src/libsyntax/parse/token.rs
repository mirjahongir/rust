// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast;
use ast::{Name, Mrk};
use ast_util;
use parse::token;
use util::interner::StrInterner;
use util::interner;

use std::cast;
use std::char;
use std::cmp::Equiv;
use std::local_data;
use std::rand;
use std::rand::RngUtil;

#[deriving(Clone, Encodable, Decodable, Eq, IterBytes)]
pub enum binop {
    PLUS,
    MINUS,
    STAR,
    SLASH,
    PERCENT,
    CARET,
    AND,
    OR,
    SHL,
    SHR,
}

#[deriving(Clone, Encodable, Decodable, Eq, IterBytes)]
pub enum Token {
    /* Expression-operator symbols. */
    EQ,
    LT,
    LE,
    EQEQ,
    NE,
    GE,
    GT,
    ANDAND,
    OROR,
    NOT,
    TILDE,
    BINOP(binop),
    BINOPEQ(binop),

    /* Structural symbols */
    AT,
    DOT,
    DOTDOT,
    COMMA,
    SEMI,
    COLON,
    MOD_SEP,
    RARROW,
    LARROW,
    DARROW,
    FAT_ARROW,
    LPAREN,
    RPAREN,
    LBRACKET,
    RBRACKET,
    LBRACE,
    RBRACE,
    POUND,
    DOLLAR,

    /* Literals */
    LIT_INT(i64, ast::int_ty),
    LIT_UINT(u64, ast::uint_ty),
    LIT_INT_UNSUFFIXED(i64),
    LIT_FLOAT(ast::ident, ast::float_ty),
    LIT_FLOAT_UNSUFFIXED(ast::ident),
    LIT_STR(ast::ident),

    /* Name components */
    // an identifier contains an "is_mod_name" boolean,
    // indicating whether :: follows this token with no
    // whitespace in between.
    IDENT(ast::ident, bool),
    UNDERSCORE,
    LIFETIME(ast::ident),

    /* For interpolation */
    INTERPOLATED(nonterminal),

    DOC_COMMENT(ast::ident),
    EOF,
}

#[deriving(Clone, Encodable, Decodable, Eq, IterBytes)]
/// For interpolation during macro expansion.
pub enum nonterminal {
    nt_item(@ast::item),
    nt_block(~ast::Block),
    nt_stmt(@ast::stmt),
    nt_pat( @ast::pat),
    nt_expr(@ast::expr),
    nt_ty(  ~ast::Ty),
    nt_ident(~ast::ident, bool),
    nt_attr(@ast::Attribute),   // #[foo]
    nt_path(~ast::Path),
    nt_tt(  @ast::token_tree), //needs @ed to break a circularity
    nt_matchers(~[ast::matcher])
}

pub fn binop_to_str(o: binop) -> ~str {
    match o {
      PLUS => ~"+",
      MINUS => ~"-",
      STAR => ~"*",
      SLASH => ~"/",
      PERCENT => ~"%",
      CARET => ~"^",
      AND => ~"&",
      OR => ~"|",
      SHL => ~"<<",
      SHR => ~">>"
    }
}

pub fn to_str(input: @ident_interner, t: &Token) -> ~str {
    match *t {
      EQ => ~"=",
      LT => ~"<",
      LE => ~"<=",
      EQEQ => ~"==",
      NE => ~"!=",
      GE => ~">=",
      GT => ~">",
      NOT => ~"!",
      TILDE => ~"~",
      OROR => ~"||",
      ANDAND => ~"&&",
      BINOP(op) => binop_to_str(op),
      BINOPEQ(op) => binop_to_str(op) + "=",

      /* Structural symbols */
      AT => ~"@",
      DOT => ~".",
      DOTDOT => ~"..",
      COMMA => ~",",
      SEMI => ~";",
      COLON => ~":",
      MOD_SEP => ~"::",
      RARROW => ~"->",
      LARROW => ~"<-",
      DARROW => ~"<->",
      FAT_ARROW => ~"=>",
      LPAREN => ~"(",
      RPAREN => ~")",
      LBRACKET => ~"[",
      RBRACKET => ~"]",
      LBRACE => ~"{",
      RBRACE => ~"}",
      POUND => ~"#",
      DOLLAR => ~"$",

      /* Literals */
      LIT_INT(c, ast::ty_char) => {
          let mut res = ~"'";
          do (c as char).escape_default |c| {
              res.push_char(c);
          }
          res.push_char('\'');
          res
      }
      LIT_INT(i, t) => {
          i.to_str() + ast_util::int_ty_to_str(t)
      }
      LIT_UINT(u, t) => {
          u.to_str() + ast_util::uint_ty_to_str(t)
      }
      LIT_INT_UNSUFFIXED(i) => { i.to_str() }
      LIT_FLOAT(ref s, t) => {
        let mut body = ident_to_str(s).to_owned();
        if body.ends_with(".") {
            body.push_char('0');  // `10.f` is not a float literal
        }
        body + ast_util::float_ty_to_str(t)
      }
      LIT_FLOAT_UNSUFFIXED(ref s) => {
        let mut body = ident_to_str(s).to_owned();
        if body.ends_with(".") {
            body.push_char('0');  // `10.f` is not a float literal
        }
        body
      }
      LIT_STR(ref s) => { fmt!("\"%s\"", ident_to_str(s).escape_default()) }

      /* Name components */
      IDENT(s, _) => input.get(s.name).to_owned(),
      LIFETIME(s) => fmt!("'%s", input.get(s.name)),
      UNDERSCORE => ~"_",

      /* Other */
      DOC_COMMENT(ref s) => ident_to_str(s).to_owned(),
      EOF => ~"<eof>",
      INTERPOLATED(ref nt) => {
        match nt {
            &nt_expr(e) => ::print::pprust::expr_to_str(e, input),
            &nt_attr(e) => ::print::pprust::attribute_to_str(e, input),
            _ => {
                ~"an interpolated " +
                    match (*nt) {
                      nt_item(*) => ~"item",
                      nt_block(*) => ~"block",
                      nt_stmt(*) => ~"statement",
                      nt_pat(*) => ~"pattern",
                      nt_attr(*) => fail!("should have been handled"),
                      nt_expr(*) => fail!("should have been handled above"),
                      nt_ty(*) => ~"type",
                      nt_ident(*) => ~"identifier",
                      nt_path(*) => ~"path",
                      nt_tt(*) => ~"tt",
                      nt_matchers(*) => ~"matcher sequence"
                    }
            }
        }
      }
    }
}

pub fn can_begin_expr(t: &Token) -> bool {
    match *t {
      LPAREN => true,
      LBRACE => true,
      LBRACKET => true,
      IDENT(_, _) => true,
      UNDERSCORE => true,
      TILDE => true,
      LIT_INT(_, _) => true,
      LIT_UINT(_, _) => true,
      LIT_INT_UNSUFFIXED(_) => true,
      LIT_FLOAT(_, _) => true,
      LIT_FLOAT_UNSUFFIXED(_) => true,
      LIT_STR(_) => true,
      POUND => true,
      AT => true,
      NOT => true,
      BINOP(MINUS) => true,
      BINOP(STAR) => true,
      BINOP(AND) => true,
      BINOP(OR) => true, // in lambda syntax
      OROR => true, // in lambda syntax
      MOD_SEP => true,
      INTERPOLATED(nt_expr(*))
      | INTERPOLATED(nt_ident(*))
      | INTERPOLATED(nt_block(*))
      | INTERPOLATED(nt_path(*)) => true,
      _ => false
    }
}

/// what's the opposite delimiter?
pub fn flip_delimiter(t: &token::Token) -> token::Token {
    match *t {
      LPAREN => RPAREN,
      LBRACE => RBRACE,
      LBRACKET => RBRACKET,
      RPAREN => LPAREN,
      RBRACE => LBRACE,
      RBRACKET => LBRACKET,
      _ => fail!()
    }
}



pub fn is_lit(t: &Token) -> bool {
    match *t {
      LIT_INT(_, _) => true,
      LIT_UINT(_, _) => true,
      LIT_INT_UNSUFFIXED(_) => true,
      LIT_FLOAT(_, _) => true,
      LIT_FLOAT_UNSUFFIXED(_) => true,
      LIT_STR(_) => true,
      _ => false
    }
}

pub fn is_ident(t: &Token) -> bool {
    match *t { IDENT(_, _) => true, _ => false }
}

pub fn is_ident_or_path(t: &Token) -> bool {
    match *t {
      IDENT(_, _) | INTERPOLATED(nt_path(*)) => true,
      _ => false
    }
}

pub fn is_plain_ident(t: &Token) -> bool {
    match *t { IDENT(_, false) => true, _ => false }
}

pub fn is_bar(t: &Token) -> bool {
    match *t { BINOP(OR) | OROR => true, _ => false }
}


pub mod special_idents {
    use ast::ident;

    pub static underscore : ident = ident { name: 0, ctxt: 0};
    pub static anon : ident = ident { name: 1, ctxt: 0};
    pub static invalid : ident = ident { name: 2, ctxt: 0}; // ''
    pub static unary : ident = ident { name: 3, ctxt: 0};
    pub static not_fn : ident = ident { name: 4, ctxt: 0};
    pub static idx_fn : ident = ident { name: 5, ctxt: 0};
    pub static unary_minus_fn : ident = ident { name: 6, ctxt: 0};
    pub static clownshoes_extensions : ident = ident { name: 7, ctxt: 0};

    pub static self_ : ident = ident { name: 8, ctxt: 0}; // 'self'

    /* for matcher NTs */
    pub static item : ident = ident { name: 9, ctxt: 0};
    pub static block : ident = ident { name: 10, ctxt: 0};
    pub static stmt : ident = ident { name: 11, ctxt: 0};
    pub static pat : ident = ident { name: 12, ctxt: 0};
    pub static expr : ident = ident { name: 13, ctxt: 0};
    pub static ty : ident = ident { name: 14, ctxt: 0};
    pub static ident : ident = ident { name: 15, ctxt: 0};
    pub static path : ident = ident { name: 16, ctxt: 0};
    pub static tt : ident = ident { name: 17, ctxt: 0};
    pub static matchers : ident = ident { name: 18, ctxt: 0};

    pub static str : ident = ident { name: 19, ctxt: 0}; // for the type

    /* outside of libsyntax */
    pub static arg : ident = ident { name: 20, ctxt: 0};
    pub static descrim : ident = ident { name: 21, ctxt: 0};
    pub static clownshoe_abi : ident = ident { name: 22, ctxt: 0};
    pub static clownshoe_stack_shim : ident = ident { name: 23, ctxt: 0};
    pub static main : ident = ident { name: 24, ctxt: 0};
    pub static opaque : ident = ident { name: 25, ctxt: 0};
    pub static blk : ident = ident { name: 26, ctxt: 0};
    pub static statik : ident = ident { name: 27, ctxt: 0};
    pub static clownshoes_foreign_mod: ident = ident { name: 28, ctxt: 0};
    pub static unnamed_field: ident = ident { name: 29, ctxt: 0};
    pub static c_abi: ident = ident { name: 30, ctxt: 0};
    pub static type_self: ident = ident { name: 31, ctxt: 0};    // `Self`
}

/**
 * Maps a token to a record specifying the corresponding binary
 * operator
 */
pub fn token_to_binop(tok: &Token) -> Option<ast::binop> {
  match *tok {
      BINOP(STAR)    => Some(ast::mul),
      BINOP(SLASH)   => Some(ast::div),
      BINOP(PERCENT) => Some(ast::rem),
      BINOP(PLUS)    => Some(ast::add),
      BINOP(MINUS)   => Some(ast::subtract),
      BINOP(SHL)     => Some(ast::shl),
      BINOP(SHR)     => Some(ast::shr),
      BINOP(AND)     => Some(ast::bitand),
      BINOP(CARET)   => Some(ast::bitxor),
      BINOP(OR)      => Some(ast::bitor),
      LT             => Some(ast::lt),
      LE             => Some(ast::le),
      GE             => Some(ast::ge),
      GT             => Some(ast::gt),
      EQEQ           => Some(ast::eq),
      NE             => Some(ast::ne),
      ANDAND         => Some(ast::and),
      OROR           => Some(ast::or),
      _              => None
  }
}

// looks like we can get rid of this completely...
pub type ident_interner = StrInterner;

// return a fresh interner, preloaded with special identifiers.
fn mk_fresh_ident_interner() -> @ident_interner {
    // the indices here must correspond to the numbers in
    // special_idents.
    let init_vec = ~[
        "_",                  // 0
        "anon",               // 1
        "",                   // 2
        "unary",              // 3
        "!",                  // 4
        "[]",                 // 5
        "unary-",             // 6
        "__extensions__",     // 7
        "self",               // 8
        "item",               // 9
        "block",              // 10
        "stmt",               // 11
        "pat",                // 12
        "expr",               // 13
        "ty",                 // 14
        "ident",              // 15
        "path",               // 16
        "tt",                 // 17
        "matchers",           // 18
        "str",                // 19
        "arg",                // 20
        "descrim",            // 21
        "__rust_abi",         // 22
        "__rust_stack_shim",  // 23
        "main",               // 24
        "<opaque>",           // 25
        "blk",                // 26
        "static",             // 27
        "__foreign_mod__",    // 28
        "__field__",          // 29
        "C",                  // 30
        "Self",               // 31

        "as",                 // 32
        "break",              // 33
        "const",              // 34
        "do",                 // 35
        "else",               // 36
        "enum",               // 37
        "extern",             // 38
        "false",              // 39
        "fn",                 // 40
        "for",                // 41
        "if",                 // 42
        "impl",               // 43
        "let",                // 44
        "__log",              // 45
        "loop",               // 46
        "match",              // 47
        "mod",                // 48
        "mut",                // 49
        "once",               // 50
        "priv",               // 51
        "pub",                // 52
        "ref",                // 53
        "return",             // 54
        "static",             // 27 -- also a special ident
        "self",               //  8 -- also a special ident
        "struct",             // 55
        "super",              // 56
        "true",               // 57
        "trait",              // 58
        "type",               // 59
        "unsafe",             // 60
        "use",                // 61
        "while",              // 62
        "in",                 // 63

        "be",                 // 64
        "pure",               // 65
        "yield",              // 66
    ];

    @interner::StrInterner::prefill(init_vec)
}

// if an interner exists in TLS, return it. Otherwise, prepare a
// fresh one.
pub fn get_ident_interner() -> @ident_interner {
    static key: local_data::Key<@@::parse::token::ident_interner> =
        &local_data::Key;
    match local_data::get(key, |k| k.map_move(|k| *k)) {
        Some(interner) => *interner,
        None => {
            let interner = mk_fresh_ident_interner();
            local_data::set(key, @interner);
            interner
        }
    }
}

/* for when we don't care about the contents; doesn't interact with TLD or
   serialization */
pub fn mk_fake_ident_interner() -> @ident_interner {
    @interner::StrInterner::new()
}

// maps a string to its interned representation
pub fn intern(str : &str) -> Name {
    let interner = get_ident_interner();
    interner.intern(str)
}

// gensyms a new uint, using the current interner
pub fn gensym(str : &str) -> Name {
    let interner = get_ident_interner();
    interner.gensym(str)
}

// map an interned representation back to a string
pub fn interner_get(name : Name) -> @str {
    get_ident_interner().get(name)
}

// maps an identifier to the string that it corresponds to
pub fn ident_to_str(id : &ast::ident) -> @str {
    interner_get(id.name)
}

// maps a string to an identifier with an empty syntax context
pub fn str_to_ident(str : &str) -> ast::ident {
    ast::new_ident(intern(str))
}

// maps a string to a gensym'ed identifier
pub fn gensym_ident(str : &str) -> ast::ident {
    ast::new_ident(gensym(str))
}

// create a fresh name that maps to the same string as the old one.
// note that this guarantees that str_ptr_eq(ident_to_str(src),interner_get(fresh_name(src)));
// that is, that the new name and the old one are connected to ptr_eq strings.
pub fn fresh_name(src : &ast::ident) -> Name {
    let interner = get_ident_interner();
    interner.gensym_copy(src.name)
    // following: debug version. Could work in final except that it's incompatible with
    // good error messages and uses of struct names in ambiguous could-be-binding
    // locations. Also definitely destroys the guarantee given above about ptr_eq.
    /*let num = rand::rng().gen_uint_range(0,0xffff);
    gensym(fmt!("%s_%u",ident_to_str(src),num))*/
}

// it looks like there oughta be a str_ptr_eq fn, but no one bothered to implement it?

// determine whether two @str values are pointer-equal
pub fn str_ptr_eq(a : @str, b : @str) -> bool {
    unsafe {
        let p : uint = cast::transmute(a);
        let q : uint = cast::transmute(b);
        let result = p == q;
        // got to transmute them back, to make sure the ref count is correct:
        let junk1 : @str = cast::transmute(p);
        let junk2 : @str = cast::transmute(q);
        result
    }
}

// return true when two identifiers refer (through the intern table) to the same ptr_eq
// string. This is used to compare identifiers in places where hygienic comparison is
// not wanted (i.e. not lexical vars).
pub fn ident_spelling_eq(a : &ast::ident, b : &ast::ident) -> bool {
    str_ptr_eq(interner_get(a.name),interner_get(b.name))
}

// create a fresh mark.
pub fn fresh_mark() -> Mrk {
    gensym("mark")
}

/**
 * All the valid words that have meaning in the Rust language.
 *
 * Rust keywords are either 'strict' or 'reserved'.  Strict keywords may not
 * appear as identifiers at all. Reserved keywords are not used anywhere in
 * the language and may not appear as identifiers.
 */
pub mod keywords {
    use ast::ident;

    pub enum Keyword {
        // Strict keywords
        As,
        Break,
        Const,
        Do,
        Else,
        Enum,
        Extern,
        False,
        Fn,
        For,
        If,
        Impl,
        In,
        Let,
        __Log,
        Loop,
        Match,
        Mod,
        Mut,
        Once,
        Priv,
        Pub,
        Ref,
        Return,
        Static,
        Self,
        Struct,
        Super,
        True,
        Trait,
        Type,
        Unsafe,
        Use,
        While,

        // Reserved keywords
        Be,
        Pure,
        Yield,
    }

    impl Keyword {
        pub fn to_ident(&self) -> ident {
            match *self {
                As => ident { name: 32, ctxt: 0 },
                Break => ident { name: 33, ctxt: 0 },
                Const => ident { name: 34, ctxt: 0 },
                Do => ident { name: 35, ctxt: 0 },
                Else => ident { name: 36, ctxt: 0 },
                Enum => ident { name: 37, ctxt: 0 },
                Extern => ident { name: 38, ctxt: 0 },
                False => ident { name: 39, ctxt: 0 },
                Fn => ident { name: 40, ctxt: 0 },
                For => ident { name: 41, ctxt: 0 },
                If => ident { name: 42, ctxt: 0 },
                Impl => ident { name: 43, ctxt: 0 },
                In => ident { name: 63, ctxt: 0 },
                Let => ident { name: 44, ctxt: 0 },
                __Log => ident { name: 45, ctxt: 0 },
                Loop => ident { name: 46, ctxt: 0 },
                Match => ident { name: 47, ctxt: 0 },
                Mod => ident { name: 48, ctxt: 0 },
                Mut => ident { name: 49, ctxt: 0 },
                Once => ident { name: 50, ctxt: 0 },
                Priv => ident { name: 51, ctxt: 0 },
                Pub => ident { name: 52, ctxt: 0 },
                Ref => ident { name: 53, ctxt: 0 },
                Return => ident { name: 54, ctxt: 0 },
                Static => ident { name: 27, ctxt: 0 },
                Self => ident { name: 8, ctxt: 0 },
                Struct => ident { name: 55, ctxt: 0 },
                Super => ident { name: 56, ctxt: 0 },
                True => ident { name: 57, ctxt: 0 },
                Trait => ident { name: 58, ctxt: 0 },
                Type => ident { name: 59, ctxt: 0 },
                Unsafe => ident { name: 60, ctxt: 0 },
                Use => ident { name: 61, ctxt: 0 },
                While => ident { name: 62, ctxt: 0 },
                Be => ident { name: 64, ctxt: 0 },
                Pure => ident { name: 65, ctxt: 0 },
                Yield => ident { name: 66, ctxt: 0 },
            }
        }
    }
}

pub fn is_keyword(kw: keywords::Keyword, tok: &Token) -> bool {
    match *tok {
        token::IDENT(sid, false) => { kw.to_ident().name == sid.name }
        _ => { false }
    }
}

pub fn is_any_keyword(tok: &Token) -> bool {
    match *tok {
        token::IDENT(sid, false) => match sid.name {
            8 | 27 | 32 .. 66 => true,
            _ => false,
        },
        _ => false
    }
}

pub fn is_strict_keyword(tok: &Token) -> bool {
    match *tok {
        token::IDENT(sid, false) => match sid.name {
            8 | 27 | 32 .. 63 => true,
            _ => false,
        },
        _ => false,
    }
}

pub fn is_reserved_keyword(tok: &Token) -> bool {
    match *tok {
        token::IDENT(sid, false) => match sid.name {
            64 .. 66 => true,
            _ => false,
        },
        _ => false,
    }
}

pub fn mtwt_token_eq(t1 : &Token, t2 : &Token) -> bool {
    if (*t1 == *t2) {
        true
    } else {
        match (t1,t2) {
            (&IDENT(id1,_),&IDENT(id2,_)) =>
            ast_util::mtwt_resolve(id1) == ast_util::mtwt_resolve(id2),
            _ => false
        }
    }
}


#[cfg(test)]
mod test {
    use ast;
    use ast_util;
    use super::*;
    use std::io;
    use std::managed;

    #[test] fn t1() {
        let a = fresh_name("ghi");
        printfln!("interned name: %u,\ntextual name: %s\n",
                  a, interner_get(a));
    }

    fn mark_ident(id : ast::ident, m : ast::Mrk) -> ast::ident {
        ast::ident{name:id.name,ctxt:ast_util::new_mark(m,id.ctxt)}
    }

    #[test] fn mtwt_token_eq_test() {
        assert!(mtwt_token_eq(&GT,&GT));
        let a = str_to_ident("bac");
        let a1 = mark_ident(a,92);
        assert!(mtwt_token_eq(&IDENT(a,true),&IDENT(a1,false)));
    }

    #[test] fn str_ptr_eq_tests(){
        let a = @"abc";
        let b = @"abc";
        let c = a;
        assert!(str_ptr_eq(a,c));
        assert!(!str_ptr_eq(a,b));
    }

    #[test] fn fresh_name_pointer_sharing() {
        let ghi = str_to_ident("ghi");
        assert_eq!(ident_to_str(&ghi),@"ghi");
        assert!(str_ptr_eq(ident_to_str(&ghi),ident_to_str(&ghi)))
        let fresh = ast::new_ident(fresh_name(&ghi));
        assert_eq!(ident_to_str(&fresh),@"ghi");
        assert!(str_ptr_eq(ident_to_str(&ghi),ident_to_str(&fresh)));
    }

}
