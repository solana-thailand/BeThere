//! Rust Adventures — interactive tile-based puzzle game.
//!
//! Teaches Rust programming through a Vim Adventures-style game:
//! grid movement, key collection, code puzzles, NPC dialogs.

pub mod engine;
pub mod page;
pub mod types;

use serde::{Deserialize, Serialize};
use types::*;

/// Built-in test level for development.
pub fn test_level() -> LevelData {
    LevelData {
        id: "test_01".to_string(),
        name: "Test Level".to_string(),
        concept: "Movement & Keys".to_string(),
        width: 12,
        height: 8,
        grid: vec![
            "############".to_string(),
            "#@.........#".to_string(),
            "#..........#".to_string(),
            "#..........#".to_string(),
            "#..........#".to_string(),
            "#..........#".to_string(),
            "#.........>#".to_string(),
            "############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (3, 2),
                name: "fn".to_string(),
                description: "fn declares a function in Rust".to_string(),
            },
            KeyDef {
                pos: (6, 4),
                name: "let".to_string(),
                description: "let binds a value to a variable".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (4, 3),
                name: "Ferris".to_string(),
                dialog: "Welcome to Rustland! I'm Ferris, your guide. Collect the keyword keys to proceed!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (8, 5),
                puzzle_id: "test_arrange".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (2, 5),
                text: "Use arrow keys or WASD to move. Collect keys, solve puzzles!".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "test_arrange".to_string(),
                instruction: "Arrange these lines into a valid Rust program:".to_string(),
                pieces: vec![
                    "}".to_string(),
                    "    println!(\"Hello, Rust!\");".to_string(),
                    "fn main() {".to_string(),
                ],
                solution: "fn main() {\n    println!(\"Hello, Rust!\");\n}".to_string(),
                hint: "Every Rust program starts with fn main(). The body goes inside curly braces.".to_string(),
            },
        ],
        required_keys: vec!["fn".to_string(), "let".to_string()],
        intro_text: "Welcome to Rustland! Collect the `fn` and `let` keys, then solve the code puzzle to open the gate.".to_string(),
        completion_text: "Well done! You wrote your first Rust program!".to_string(),
    }
}

/// Level 1: Hello World — learn `fn`, `let`, `println!`
pub fn level_01_hello_world() -> LevelData {
    LevelData {
        id: "01_hello_world".to_string(),
        name: "Hello, Rust!".to_string(),
        concept: "fn, let, println!".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#.###........#".to_string(),
            "#............#".to_string(),
            "#............#".to_string(),
            "#.....####...#".to_string(),
            "#............#".to_string(),
            "#............#".to_string(),
            "#...........>#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (3, 4),
                name: "fn".to_string(),
                description: "fn declares a function in Rust".to_string(),
            },
            KeyDef {
                pos: (8, 2),
                name: "let".to_string(),
                description: "let binds a value to a variable".to_string(),
            },
            KeyDef {
                pos: (11, 6),
                name: "println!".to_string(),
                description: "println! prints a line to stdout".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (5, 1),
                name: "Ferris".to_string(),
                dialog: "Every Rust program needs a `fn main()` function — it's where execution begins!".to_string(),
            },
            NpcDef {
                pos: (5, 4),
                name: "Ferris's Friend".to_string(),
                dialog: "`println!` is a macro (notice the !) that prints text. Try: println!(\"Hello!\");".to_string(),
            },
        ],
        gates: vec![GateDef {
            pos: (12, 6),
            puzzle_id: "l01_arrange_hello".to_string(),
        }],
        signs: vec![
            SignDef {
                pos: (2, 1),
                text: "Use arrow keys or WASD to move around.".to_string(),
            },
            SignDef {
                pos: (4, 7),
                text: "Collect keyword keys to unlock the exit!".to_string(),
            },
        ],
        puzzles: vec![PuzzleDef::Arrange {
            id: "l01_arrange_hello".to_string(),
            instruction: "Arrange these lines into a valid Hello World program:".to_string(),
            pieces: vec![
                "}".to_string(),
                "    println!(\"Hello, world!\");".to_string(),
                "fn main() {".to_string(),
            ],
            solution: "fn main() {\n    println!(\"Hello, world!\");\n}".to_string(),
            hint: "Every Rust program starts with fn main(). The body goes inside curly braces.".to_string(),
        }],
        required_keys: vec!["fn".to_string(), "let".to_string(), "println!".to_string()],
        intro_text: "Welcome to Rustland! Collect the `fn`, `let`, and `println!` keys, then solve the code puzzle to open the gate.".to_string(),
        completion_text: "Well done! You wrote your first Rust program!".to_string(),
    }
}

/// Level 2: Variables — learn `mut`, `const`, shadowing
pub fn level_02_variables() -> LevelData {
    LevelData {
        id: "02_variables".to_string(),
        name: "Variables & Mutability".to_string(),
        concept: "mut, const, shadowing".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#..####......#".to_string(),
            "#..#.........#".to_string(),
            "#..#..####...#".to_string(),
            "#..#.........#".to_string(),
            "#..####..###.#".to_string(),
            "#...........>#".to_string(),
            "#............#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (5, 3),
                name: "mut".to_string(),
                description: "mut makes a variable mutable (changeable)".to_string(),
            },
            KeyDef {
                pos: (9, 5),
                name: "const".to_string(),
                description: "const declares a compile-time constant".to_string(),
            },
        ],
        npcs: vec![NpcDef {
            pos: (3, 7),
            name: "Ferris".to_string(),
            dialog:
                "By default, variables are immutable in Rust. Use `mut` to make them changeable!"
                    .to_string(),
        }],
        gates: vec![
            GateDef {
                pos: (7, 5),
                puzzle_id: "l02_fix_type_error".to_string(),
            },
            GateDef {
                pos: (7, 6),
                puzzle_id: "l02_fill_blank_let".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (1, 4),
                text: "Variables are immutable by default in Rust.".to_string(),
            },
            SignDef {
                pos: (10, 8),
                text: "Shadowing lets you re-declare a variable with the same name.".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::FixError {
                id: "l02_fix_type_error".to_string(),
                instruction: "This code has a type mismatch. Pick the correct fix:".to_string(),
                broken_code: "let x: i32 = \"hello\";".to_string(),
                options: vec![
                    "let x: &str = \"hello\";".to_string(),
                    "let x: i32 = 42;".to_string(),
                    "let x: String = \"hello\";".to_string(),
                ],
                answer: "let x: &str = \"hello\";".to_string(),
                hint: "The type annotation i32 doesn't match the string literal.".to_string(),
            },
            PuzzleDef::FillBlank {
                id: "l02_fill_blank_let".to_string(),
                instruction: "Fill in the blank to declare a mutable variable:".to_string(),
                code_template: "___ x = 5;\nx = 10; // This should compile".to_string(),
                blank: "___".to_string(),
                options: vec![
                    "let".to_string(),
                    "let mut".to_string(),
                    "const".to_string(),
                ],
                answer: "let mut".to_string(),
                hint: "Variables are immutable by default. What keyword makes them changeable?"
                    .to_string(),
            },
        ],
        required_keys: vec!["mut".to_string(), "const".to_string()],
        intro_text:
            "Learn about variables! Collect the `mut` and `const` keys, then solve the puzzles."
                .to_string(),
        completion_text: "Great work! You understand Rust variables and mutability!".to_string(),
    }
}

/// Level 3: Types — learn basic Rust types and type inference.
pub fn level_03_types() -> LevelData {
    LevelData {
        id: "03_types".to_string(),
        name: "Types & Type Inference".to_string(),
        concept: "i32, f64, bool, char, String, &str".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#...##......'#".to_string(),
            "#.....#.....'#".to_string(),
            "#.####..##...#".to_string(),
            "#..........#.#".to_string(),
            "#..##...#....#".to_string(),
            "#.....#.....'#".to_string(),
            "#..........#>#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (3, 2),
                name: "i32".to_string(),
                description: "Signed 32-bit integer type".to_string(),
            },
            KeyDef {
                pos: (9, 1),
                name: "String".to_string(),
                description: "Heap-allocated string type".to_string(),
            },
            KeyDef {
                pos: (3, 7),
                name: "bool".to_string(),
                description: "Boolean type — true or false".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (5, 3),
                name: "Ferris".to_string(),
                dialog: "Rust is statically typed, but the compiler is smart! It can infer types from context — you often don't need to write them explicitly.".to_string(),
            },
            NpcDef {
                pos: (9, 6),
                name: "Type Checker".to_string(),
                dialog: "Every value in Rust has exactly one type. The compiler checks types at compile time — no runtime surprises!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (7, 5),
                puzzle_id: "l03_match_types".to_string(),
            },
            GateDef {
                pos: (12, 7),
                puzzle_id: "l03_fix_type".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (2, 1),
                text: "Rust has two string types: &str (borrowed slice) and String (heap-allocated).".to_string(),
            },
            SignDef {
                pos: (10, 8),
                text: "Type inference means the compiler figures out types for you when it can.".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::MatchPairs {
                id: "l03_match_types".to_string(),
                instruction: "Match each Rust type with its description:".to_string(),
                pairs: vec![
                    ("i32".to_string(), "Signed 32-bit integer".to_string()),
                    ("f64".to_string(), "64-bit floating point".to_string()),
                    ("bool".to_string(), "true or false".to_string()),
                    ("&str".to_string(), "String slice".to_string()),
                    ("String".to_string(), "Heap-allocated string".to_string()),
                    ("char".to_string(), "Unicode scalar value".to_string()),
                ],
                hint: "i32 and f64 are number types. &str is borrowed, String is owned.".to_string(),
            },
            PuzzleDef::FixError {
                id: "l03_fix_type".to_string(),
                instruction: "This code has a type error. Pick the correct fix:".to_string(),
                broken_code: "let x: i32 = 3.14;".to_string(),
                options: vec![
                    "let x: f64 = 3.14;".to_string(),
                    "let x: i32 = 3;".to_string(),
                    "let x = 3.14;".to_string(),
                ],
                answer: "let x: f64 = 3.14;".to_string(),
                hint: "3.14 is a floating point number. i32 can't hold decimals.".to_string(),
            },
        ],
        required_keys: vec!["i32".to_string(), "String".to_string(), "bool".to_string()],
        intro_text: "Rust has a powerful type system! Collect the `i32`, `String`, and `bool` keys, then match types to their descriptions.".to_string(),
        completion_text: "Excellent! You understand Rust's basic types and type inference!".to_string(),
    }
}

/// Level 4: Control Flow — learn if, match, for loops.
pub fn level_04_control_flow() -> LevelData {
    LevelData {
        id: "04_control_flow".to_string(),
        name: "Control Flow".to_string(),
        concept: "if, else, match, loop, for, while".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#.####.......#".to_string(),
            "#...#........#".to_string(),
            "#######.######".to_string(),
            "#.........#..#".to_string(),
            "#....#...#...#".to_string(),
            "#..........#.#".to_string(),
            "#..........#>#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (8, 2),
                name: "if".to_string(),
                description: "if starts a conditional branch in Rust".to_string(),
            },
            KeyDef {
                pos: (10, 3),
                name: "match".to_string(),
                description: "match performs pattern matching on values".to_string(),
            },
            KeyDef {
                pos: (3, 6),
                name: "for".to_string(),
                description: "for iterates over anything implementing IntoIterator".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (3, 3),
                name: "Ferris".to_string(),
                dialog: "if expressions in Rust work like other languages, but they're expressions \u{2014} they return a value! No ternary operator needed.".to_string(),
            },
            NpcDef {
                pos: (9, 5),
                name: "Loop Master".to_string(),
                dialog: "Rust has three loops: loop (infinite until break), while (conditional), and for (iterator-based). for is the most common!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (7, 4),
                puzzle_id: "l04_arrange_if".to_string(),
            },
            GateDef {
                pos: (12, 7),
                puzzle_id: "l04_fill_match".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (2, 1),
                text: "match is Rust's pattern matching powerhouse \u{2014} like a super-powered switch statement.".to_string(),
            },
            SignDef {
                pos: (10, 8),
                text: "for loops iterate over anything that implements IntoIterator \u{2014} vectors, ranges, strings!".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l04_arrange_if".to_string(),
                instruction: "Arrange these lines into a valid if/else expression:".to_string(),
                pieces: vec![
                    "    } else {".to_string(),
                    "    println!(\"x is positive\");".to_string(),
                    "    println!(\"x is zero or negative\");".to_string(),
                    "if x > 0 {".to_string(),
                    "}".to_string(),
                ],
                solution: "if x > 0 {\n    println!(\"x is positive\");\n} else {\n    println!(\"x is zero or negative\");\n}".to_string(),
                hint: "if starts the condition. else comes after the first closing brace. Each branch's body is indented.".to_string(),
            },
            PuzzleDef::FillBlank {
                id: "l04_fill_match".to_string(),
                instruction: "Fill in the blank to complete this match expression:".to_string(),
                code_template: "match number {\n    ___ => println!(\"Zero!\"),\n    1 => println!(\"One!\"),\n    _ => println!(\"Something else\"),\n}".to_string(),
                blank: "___".to_string(),
                options: vec![
                    "0".to_string(),
                    "0..=1".to_string(),
                    "\"zero\"".to_string(),
                ],
                answer: "0".to_string(),
                hint: "Match arms match literal values. What number means zero?".to_string(),
            },
        ],
        required_keys: vec!["if".to_string(), "match".to_string(), "for".to_string()],
        intro_text: "Master Rust's control flow! Collect the `if`, `match`, and `for` keys, then solve puzzles about conditionals and pattern matching.".to_string(),
        completion_text: "Great work! You can now control the flow of your Rust programs with if, match, and loops!".to_string(),
    }
}

/// Level 5: Functions — learn pub, ->, return, parameters, closures.
pub fn level_05_functions() -> LevelData {
    LevelData {
        id: "05_functions".to_string(),
        name: "Functions".to_string(),
        concept: "pub, ->, parameters, return, closures".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#....#......'#".to_string(),
            "#....#......'#".to_string(),
            "#.###..####.'#".to_string(),
            "#.....#...#.'#".to_string(),
            "#...###.....'#".to_string(),
            "#............#".to_string(),
            "#...........>#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (9, 2),
                name: "pub".to_string(),
                description: "pub makes items visible outside their module".to_string(),
            },
            KeyDef {
                pos: (3, 5),
                name: "->".to_string(),
                description: "-> specifies the return type of a function".to_string(),
            },
            KeyDef {
                pos: (11, 3),
                name: "return".to_string(),
                description: "return exits a function early with a value".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (4, 2),
                name: "Ferris".to_string(),
                dialog: "Functions in Rust are declared with `fn`. Parameters must have types, and you specify return types with `->`.".to_string(),
            },
            NpcDef {
                pos: (8, 6),
                name: "Function Fairy".to_string(),
                dialog: "The last expression in a function body is the return value — no `return` keyword needed! Unless you want early returns.".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (6, 4),
                puzzle_id: "l05_arrange_fn".to_string(),
            },
            GateDef {
                pos: (9, 6),
                puzzle_id: "l05_fix_visibility".to_string(),
            },
            GateDef {
                pos: (12, 7),
                puzzle_id: "l05_fill_return".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (2, 1),
                text: "`pub` makes items visible outside their module. Without it, items are private by default.".to_string(),
            },
            SignDef {
                pos: (11, 8),
                text: "Closures are anonymous functions: `|x| x + 1`. They can capture their environment!".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l05_arrange_fn".to_string(),
                instruction: "Arrange these lines into a complete Rust function:".to_string(),
                pieces: vec![
                    "}".to_string(),
                    "    a + b".to_string(),
                    "fn add(a: i32, b: i32) -> i32 {".to_string(),
                ],
                solution: "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".to_string(),
                hint: "Function signature first (fn name(params) -> return_type), then body, then closing brace. The last expression in a function is implicitly returned.".to_string(),
            },
            PuzzleDef::FixError {
                id: "l05_fix_visibility".to_string(),
                instruction: "This function should be public. Pick the correct fix:".to_string(),
                broken_code: "fn greet() -> String {\n    \"Hello!\".to_string()\n}".to_string(),
                options: vec![
                    "pub fn greet() -> String {\n    \"Hello!\".to_string()\n}".to_string(),
                    "public fn greet() -> String {\n    \"Hello!\".to_string()\n}".to_string(),
                    "fn pub greet() -> String {\n    \"Hello!\".to_string()\n}".to_string(),
                ],
                answer: "pub fn greet() -> String {\n    \"Hello!\".to_string()\n}".to_string(),
                hint: "In Rust, `pub` goes before `fn` to make a function public.".to_string(),
            },
            PuzzleDef::FillBlank {
                id: "l05_fill_return".to_string(),
                instruction: "Fill in the blank to specify the return type:".to_string(),
                code_template: "fn square(x: i32) ___ {\n    x * x\n}".to_string(),
                blank: "___".to_string(),
                options: vec![
                    "-> i32".to_string(),
                    ": i32".to_string(),
                    "=> i32".to_string(),
                ],
                answer: "-> i32".to_string(),
                hint: "Rust uses `->` to specify the return type, not `:` like parameter types.".to_string(),
            },
        ],
        required_keys: vec!["pub".to_string(), "->".to_string(), "return".to_string()],
        intro_text: "Master Rust functions! Collect the `pub`, `->`, and `return` keys, then solve puzzles about function signatures.".to_string(),
        completion_text: "Excellent! You now understand Rust functions — parameters, return types, visibility, and expression-based returns!".to_string(),
    }
}

/// Level 6: Ownership — learn move, borrow, clone.
pub fn level_06_ownership() -> LevelData {
    LevelData {
        id: "06_ownership".to_string(),
        name: "Ownership".to_string(),
        concept: "move, borrow, clone, references".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#.####.......#".to_string(),
            "#....#...~~..#".to_string(),
            "#.####...~~..#".to_string(),
            "#............#".to_string(),
            "#..###...##..#".to_string(),
            "#....##......#".to_string(),
            "#...........>#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (6, 1),
                name: "&".to_string(),
                description: "& creates an immutable borrow (reference)".to_string(),
            },
            KeyDef {
                pos: (3, 3),
                name: "&mut".to_string(),
                description: "&mut creates a mutable borrow".to_string(),
            },
            KeyDef {
                pos: (10, 2),
                name: "clone".to_string(),
                description: ".clone() creates a deep copy of a value".to_string(),
            },
            KeyDef {
                pos: (3, 7),
                name: "move".to_string(),
                description: "Ownership is transferred (moved) by default".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (8, 1),
                name: "Ferris".to_string(),
                dialog: "Every value in Rust has exactly one owner. When the owner goes out of scope, the value is dropped!".to_string(),
            },
            NpcDef {
                pos: (4, 5),
                name: "Borrow Checker".to_string(),
                dialog: "You can have any number of immutable references (&T), OR exactly one mutable reference (&mut T), but never both at the same time!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (7, 4),
                puzzle_id: "l06_arrange_ownership".to_string(),
            },
            GateDef {
                pos: (11, 7),
                puzzle_id: "l06_fix_move".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (1, 4),
                text: "Passing a value to a function moves ownership. Use & to borrow instead!".to_string(),
            },
            SignDef {
                pos: (11, 8),
                text: ".clone() creates a new owned copy — useful when you need to keep the original.".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l06_arrange_ownership".to_string(),
                instruction: "Arrange these lines to demonstrate borrowing:".to_string(),
                pieces: vec![
                    "    println!(\"{}\", greeting);".to_string(),
                    "let greeting = String::from(\"hello\");".to_string(),
                    "let greeting_ref = &greeting;".to_string(),
                    "}".to_string(),
                    "fn main() {".to_string(),
                ],
                solution: "fn main() {\n    let greeting = String::from(\"hello\");\n    let greeting_ref = &greeting;\n    println!(\"{}\", greeting);\n}".to_string(),
                hint: "Declare the String first, then borrow it with &. After borrowing, the owner can still be used since it's an immutable borrow.".to_string(),
            },
            PuzzleDef::FixError {
                id: "l06_fix_move".to_string(),
                instruction: "This code won't compile because `s` was moved. Pick the fix:".to_string(),
                broken_code: "let s = String::from(\"hello\");\nlet t = s;\nprintln!(\"{}\", s);".to_string(),
                options: vec![
                    "let s = String::from(\"hello\");\nlet t = s.clone();\nprintln!(\"{}\", s);".to_string(),
                    "let s = String::from(\"hello\");\nlet t = &s;\nprintln!(\"{}\", s);".to_string(),
                    "let s = String::from(\"hello\");\nlet t = s;\nprintln!(\"{}\", t);".to_string(),
                ],
                answer: "let s = String::from(\"hello\");\nlet t = s.clone();\nprintln!(\"{}\", s);".to_string(),
                hint: "Assigning a String moves it. Use .clone() to create a copy so you keep the original.".to_string(),
            },
        ],
        required_keys: vec!["&".to_string(), "&mut".to_string(), "clone".to_string(), "move".to_string()],
        intro_text: "Welcome to Rust's most unique feature — ownership! Collect the `&`, `&mut`, `clone`, and `move` keys to learn how Rust manages memory without a garbage collector.".to_string(),
        completion_text: "Amazing! You understand Rust's ownership system — the borrow checker is now your friend, not your enemy!".to_string(),
    }
}

/// Level 7: Structs & Enums — learn data structures.
pub fn level_07_structs_enums() -> LevelData {
    LevelData {
        id: "07_structs_enums".to_string(),
        name: "Structs & Enums".to_string(),
        concept: "struct, enum, impl, methods".to_string(),
        width: 15,
        height: 10,
        grid: vec![
            "###############".to_string(),
            "#@...#........#".to_string(),
            "#....#..####..#".to_string(),
            "#.####..#.....#".to_string(),
            "#........#..#.#".to_string(),
            "#...##...#..#.#".to_string(),
            "#...##......#.#".to_string(),
            "#..........#..#".to_string(),
            "#...........#>#".to_string(),
            "###############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (3, 1),
                name: "struct".to_string(),
                description: "struct defines a custom data structure".to_string(),
            },
            KeyDef {
                pos: (7, 4),
                name: "enum".to_string(),
                description: "enum defines a type that can be one of several variants".to_string(),
            },
            KeyDef {
                pos: (3, 6),
                name: "impl".to_string(),
                description: "impl adds methods to a struct or enum".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (8, 1),
                name: "Ferris".to_string(),
                dialog: "Structs group related data together, like a record. Enums let a value be one of several variants — perfect for modeling choices!".to_string(),
            },
            NpcDef {
                pos: (7, 5),
                name: "Struct Smith".to_string(),
                dialog: "Use `impl` blocks to add methods to your structs. The first parameter is `&self` for methods that read, `&mut self` for methods that modify.".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (10, 3),
                puzzle_id: "l07_arrange_struct".to_string(),
            },
            GateDef {
                pos: (12, 7),
                puzzle_id: "l07_match_enum".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (1, 2),
                text: "Struct fields are private by default in a module. Use `pub` to make them accessible.".to_string(),
            },
            SignDef {
                pos: (11, 8),
                text: "Enum variants can hold data: `enum Shape { Circle(f64), Rect(f64, f64) }`".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l07_arrange_struct".to_string(),
                instruction: "Arrange these lines into a complete struct definition with a method:".to_string(),
                pieces: vec![
                    "    name: String,".to_string(),
                    "}".to_string(),
                    "struct Player {".to_string(),
                    "    age: u32,".to_string(),
                ],
                solution: "struct Player {\n    name: String,\n    age: u32,\n}".to_string(),
                hint: "Start with `struct Name {`, then list fields with `name: Type,`, then close with `}`.".to_string(),
            },
            PuzzleDef::MatchPairs {
                id: "l07_match_enum".to_string(),
                instruction: "Match each enum variant with its data type:".to_string(),
                pairs: vec![
                    ("Option::Some(T)".to_string(), "Contains a value T".to_string()),
                    ("Option::None".to_string(), "No value present".to_string()),
                    ("Result::Ok(T)".to_string(), "Operation succeeded with T".to_string()),
                    ("Result::Err(E)".to_string(), "Operation failed with error E".to_string()),
                ],
                hint: "Option represents optional values. Result represents success or failure.".to_string(),
            },
        ],
        required_keys: vec!["struct".to_string(), "enum".to_string(), "impl".to_string()],
        intro_text: "Time to build custom data types! Collect the `struct`, `enum`, and `impl` keys, then construct objects to open gates.".to_string(),
        completion_text: "Well crafted! You can now define structs to group data and enums to represent choices — the building blocks of Rust programs!".to_string(),
    }
}

/// Level 8: Pattern Matching — learn destructuring, match arms, guards.
pub fn level_08_pattern_matching() -> LevelData {
    LevelData {
        id: "08_pattern_matching".to_string(),
        name: "Pattern Matching".to_string(),
        concept: "match, destructuring, guards, Some, None, Ok, Err".to_string(),
        width: 14,
        height: 10,
        grid: vec![
            "##############".to_string(),
            "#@...........#".to_string(),
            "#.###........#".to_string(),
            "#...#..###...#".to_string(),
            "#.##.....#...#".to_string(),
            "#....##..#...#".to_string(),
            "#....#.......#".to_string(),
            "#.##.........#".to_string(),
            "#..........#>#".to_string(),
            "##############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (5, 1),
                name: "Some".to_string(),
                description: "Some wraps a value in an Option".to_string(),
            },
            KeyDef {
                pos: (7, 4),
                name: "None".to_string(),
                description: "None represents the absence of a value".to_string(),
            },
            KeyDef {
                pos: (6, 2),
                name: "Ok".to_string(),
                description: "Ok wraps a success value in a Result".to_string(),
            },
            KeyDef {
                pos: (3, 6),
                name: "Err".to_string(),
                description: "Err wraps an error value in a Result".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (8, 1),
                name: "Ferris".to_string(),
                dialog: "Rust's `match` is exhaustive — you must handle every possible case. The compiler won't let you forget!".to_string(),
            },
            NpcDef {
                pos: (8, 6),
                name: "Pattern Master".to_string(),
                dialog: "You can destructure in match arms: `Some(x) => use(x)`, `None => handle_missing()`. You can also add guards with `if` conditions!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (6, 3),
                puzzle_id: "l08_arrange_match".to_string(),
            },
            GateDef {
                pos: (11, 7),
                puzzle_id: "l08_fill_match".to_string(),
            },
            GateDef {
                pos: (4, 7),
                puzzle_id: "l08_short_match".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (1, 3),
                text: "`match` lets you compare a value against patterns and run code based on which pattern matches.".to_string(),
            },
            SignDef {
                pos: (10, 8),
                text: "Use `_` as a catch-all pattern when you don't care about the specific value.".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l08_arrange_match".to_string(),
                instruction: "Arrange these lines into a complete match expression:".to_string(),
                pieces: vec![
                    "    Ok(n) => println!(\"Got: {}\", n),".to_string(),
                    "    Err(e) => println!(\"Error: {}\", e),".to_string(),
                    "match result {".to_string(),
                    "}".to_string(),
                ],
                solution: "match result {\n    Ok(n) => println!(\"Got: {}\", n),\n    Err(e) => println!(\"Error: {}\", e),\n}".to_string(),
                hint: "match starts the expression. Each arm is `pattern => expression,`. Don't forget the closing brace.".to_string(),
            },
            PuzzleDef::FillBlank {
                id: "l08_fill_match".to_string(),
                instruction: "Fill in the blank to extract the value from Some:".to_string(),
                code_template: "let x = Some(42);\nmatch x {\n    ___ => println!(\"Value is {}\", v),\n    None => println!(\"No value\"),\n}".to_string(),
                blank: "___".to_string(),
                options: vec![
                    "Some(v)".to_string(),
                    "Some".to_string(),
                    "v".to_string(),
                ],
                answer: "Some(v)".to_string(),
                hint: "Use `Some(identifier)` to destructure and bind the inner value to a name.".to_string(),
            },
            PuzzleDef::ShortAnswer {
                id: "l08_short_match".to_string(),
                instruction: "What keyword does Rust use as a catch-all pattern that matches anything?".to_string(),
                code_template: "match value {\n    1 => \"one\",\n    2 => \"two\",\n    ___ => \"other\",\n}".to_string(),
                answer: "_".to_string(),
                hint: "It's a single underscore character — the wildcard pattern.".to_string(),
            },
        ],
        required_keys: vec!["Some".to_string(), "None".to_string(), "Ok".to_string(), "Err".to_string()],
        intro_text: "Master Rust's pattern matching! Collect the `Some`, `None`, `Ok`, and `Err` keys, then defeat bugs by matching patterns correctly.".to_string(),
        completion_text: "Pattern matching mastered! You can now destructure enums, match exhaustively, and handle every case the Rust way!".to_string(),
    }
}

/// Level 9: Error Handling — learn Result, Option, ? operator.
pub fn level_09_error_handling() -> LevelData {
    LevelData {
        id: "09_error_handling".to_string(),
        name: "Error Handling".to_string(),
        concept: "Result, Option, ? operator, unwrap".to_string(),
        width: 15,
        height: 10,
        grid: vec![
            "###############".to_string(),
            "#@....#.......#".to_string(),
            "#.....#..###..#".to_string(),
            "#.#####..#....#".to_string(),
            "#........#..#.#".to_string(),
            "#...##........#".to_string(),
            "#...##..#.....#".to_string(),
            "#..#..........#".to_string(),
            "#............>#".to_string(),
            "###############".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (5, 1),
                name: "Result".to_string(),
                description: "Result<T, E> represents success (Ok) or failure (Err)".to_string(),
            },
            KeyDef {
                pos: (8, 3),
                name: "?".to_string(),
                description: "? propagates errors — returns Err early or unwaps Ok".to_string(),
            },
            KeyDef {
                pos: (3, 5),
                name: "unwrap".to_string(),
                description: ".unwrap() panics on Err/None, returns the Ok/Some value".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (8, 1),
                name: "Ferris".to_string(),
                dialog: "Rust doesn't have exceptions! Instead, errors are values returned via `Result<T, E>`. The `?` operator propagates them automatically.".to_string(),
            },
            NpcDef {
                pos: (10, 4),
                name: "Error Handler".to_string(),
                dialog: "`.unwrap()` is fine for prototypes and tests, but in production code, handle errors explicitly with `match` or the `?` operator!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (7, 2),
                puzzle_id: "l09_arrange_result".to_string(),
            },
            GateDef {
                pos: (8, 5),
                puzzle_id: "l09_fix_error".to_string(),
            },
            GateDef {
                pos: (12, 7),
                puzzle_id: "l09_fill_question".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (1, 4),
                text: "`Result<T, E>` is either `Ok(value)` or `Err(error)`. The `?` operator unwraps Ok or returns Err.".to_string(),
            },
            SignDef {
                pos: (11, 8),
                text: "Functions using `?` must return `Result` or `Option` — the error type must be compatible.".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l09_arrange_result".to_string(),
                instruction: "Arrange these lines into a function that handles errors with ? operator:".to_string(),
                pieces: vec![
                    "}".to_string(),
                    "    let content = read_file(path)?;".to_string(),
                    "fn read_config(path: &str) -> Result<String, std::io::Error> {".to_string(),
                    "    Ok(content)".to_string(),
                ],
                solution: "fn read_config(path: &str) -> Result<String, std::io::Error> {\n    let content = read_file(path)?;\n    Ok(content)\n}".to_string(),
                hint: "Function signature first, then the body with ? for error propagation, then wrap in Ok, then close brace.".to_string(),
            },
            PuzzleDef::FixError {
                id: "l09_fix_error".to_string(),
                instruction: "This code uses unwrap which could panic. Pick the safer version:".to_string(),
                broken_code: "let val = maybe_number.unwrap();".to_string(),
                options: vec![
                    "let val = match maybe_number {\n    Some(n) => n,\n    None => 0,\n};".to_string(),
                    "let val = maybe_number.some();".to_string(),
                    "let val = maybe_number.try();".to_string(),
                ],
                answer: "let val = match maybe_number {\n    Some(n) => n,\n    None => 0,\n};".to_string(),
                hint: "Use match to handle both Some and None cases explicitly instead of unwrap.".to_string(),
            },
            PuzzleDef::FillBlank {
                id: "l09_fill_question".to_string(),
                instruction: "Fill in the blank — what operator propagates errors in Rust?".to_string(),
                code_template: "fn get_length(s: &str) -> Result<usize, ParseError> {\n    let n: usize = parse(s)___;\n    Ok(n)\n}".to_string(),
                blank: "___".to_string(),
                options: vec![
                    "?".to_string(),
                    ".unwrap()".to_string(),
                    "!".to_string(),
                ],
                answer: "?".to_string(),
                hint: "The `?` operator unwraps Ok or propagates Err. It's Rust's way of error propagation.".to_string(),
            },
        ],
        required_keys: vec!["Result".to_string(), "?".to_string(), "unwrap".to_string()],
        intro_text: "Learn Rust's approach to errors — no exceptions, just values! Collect the `Result`, `?`, and `unwrap` keys to handle errors like a pro.".to_string(),
        completion_text: "Excellent! You now know how Rust handles errors — with Result, Option, and the ? operator. No more panics in production!".to_string(),
    }
}

/// Level 10: Traits — learn trait definitions, impl, derive.
pub fn level_10_traits() -> LevelData {
    LevelData {
        id: "10_traits".to_string(),
        name: "Traits".to_string(),
        concept: "trait, impl, derive, trait bounds".to_string(),
        width: 16,
        height: 11,
        grid: vec![
            "################".to_string(),
            "#@..#..........#".to_string(),
            "#...#..####....#".to_string(),
            "#.####..#......#".to_string(),
            "#......#..#.#..#".to_string(),
            "#...##..#....#.#".to_string(),
            "#...##......#..#".to_string(),
            "#...#.........>#".to_string(),
            "#.............##".to_string(),
            "#..............#".to_string(),
            "################".to_string(),
        ],
        keys: vec![
            KeyDef {
                pos: (6, 1),
                name: "trait".to_string(),
                description: "trait defines shared behavior (like an interface)".to_string(),
            },
            KeyDef {
                pos: (9, 4),
                name: "impl".to_string(),
                description: "impl implements a trait for a type".to_string(),
            },
            KeyDef {
                pos: (3, 5),
                name: "derive".to_string(),
                description: "#[derive(...)] auto-generates trait implementations".to_string(),
            },
        ],
        npcs: vec![
            NpcDef {
                pos: (8, 1),
                name: "Ferris".to_string(),
                dialog: "Traits are Rust's way of defining shared behavior — think of them as interfaces. Any type can implement a trait!".to_string(),
            },
            NpcDef {
                pos: (10, 6),
                name: "Trait Wizard".to_string(),
                dialog: "`#[derive(Debug, Clone, PartialEq)]` auto-generates common trait implementations. You can derive: Debug, Clone, Copy, PartialEq, Eq, Hash, and more!".to_string(),
            },
        ],
        gates: vec![
            GateDef {
                pos: (6, 3),
                puzzle_id: "l10_arrange_trait".to_string(),
            },
            GateDef {
                pos: (9, 5),
                puzzle_id: "l10_fix_trait".to_string(),
            },
            GateDef {
                pos: (13, 8),
                puzzle_id: "l10_fill_derive".to_string(),
            },
        ],
        signs: vec![
            SignDef {
                pos: (1, 3),
                text: "Traits define method signatures. Types implement those methods. This is Rust's approach to polymorphism.".to_string(),
            },
            SignDef {
                pos: (13, 9),
                text: "Trait bounds constrain generics: `fn print<T: Display>(item: T)` means T must implement Display.".to_string(),
            },
        ],
        puzzles: vec![
            PuzzleDef::Arrange {
                id: "l10_arrange_trait".to_string(),
                instruction: "Arrange these lines to define and implement a trait:".to_string(),
                pieces: vec![
                    "    fn describe(&self) -> String;".to_string(),
                    "}".to_string(),
                    "trait Describable {".to_string(),
                    "    fn describe(&self) -> String {".to_string(),
                    "impl Describable for Player {".to_string(),
                    "        format!(\"Player: {}\", self.name)".to_string(),
                    "    }".to_string(),
                    "}".to_string(),
                ],
                solution: "trait Describable {\n    fn describe(&self) -> String;\n}\nimpl Describable for Player {\n    fn describe(&self) -> String {\n        format!(\"Player: {}\", self.name)\n    }\n}".to_string(),
                hint: "First define the trait with its method signature. Then implement it for a specific type with the full method body.".to_string(),
            },
            PuzzleDef::FixError {
                id: "l10_fix_trait".to_string(),
                instruction: "This struct can't be printed with debug format. Pick the correct fix:".to_string(),
                broken_code: "struct Point {\n    x: f64,\n    y: f64,\n}".to_string(),
                options: vec![
                    "#[derive(Debug)]\nstruct Point {\n    x: f64,\n    y: f64,\n}".to_string(),
                    "#[debug]\nstruct Point {\n    x: f64,\n    y: f64,\n}".to_string(),
                    "struct Point implements Debug {\n    x: f64,\n    y: f64,\n}".to_string(),
                ],
                answer: "#[derive(Debug)]\nstruct Point {\n    x: f64,\n    y: f64,\n}".to_string(),
                hint: "Use `#[derive(Debug)]` attribute above the struct to auto-generate the Debug trait implementation.".to_string(),
            },
            PuzzleDef::FillBlank {
                id: "l10_fill_derive".to_string(),
                instruction: "Fill in the blank to add a trait bound to this generic function:".to_string(),
                code_template: "fn print_item<T: ___>(item: T) {\n    println!(\"{}\", item);\n}".to_string(),
                blank: "___".to_string(),
                options: vec![
                    "Display".to_string(),
                    "Debug".to_string(),
                    "Print".to_string(),
                ],
                answer: "Display".to_string(),
                hint: "The `{}` format specifier requires the `Display` trait. `Debug` uses `{:?}` instead.".to_string(),
            },
        ],
        required_keys: vec!["trait".to_string(), "impl".to_string(), "derive".to_string()],
        intro_text: "The final challenge — traits! Collect the `trait`, `impl`, and `derive` keys, then implement traits to open the final door.".to_string(),
        completion_text: "Congratulations! You've completed all 10 levels and mastered Rust fundamentals — from Hello World to Traits! You are now a true Rustacean!".to_string(),
    }
}

/// All built-in levels (for development — production loads from KV).
pub fn default_levels() -> Vec<LevelData> {
    vec![
        level_01_hello_world(),
        level_02_variables(),
        level_03_types(),
        level_04_control_flow(),
        level_05_functions(),
        level_06_ownership(),
        level_07_structs_enums(),
        level_08_pattern_matching(),
        level_09_error_handling(),
        level_10_traits(),
    ]
}

/// Adventure configuration (stored in KV per event).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdventureConfig {
    /// List of levels for this event's adventure.
    pub levels: Vec<LevelData>,
}

/// Adventure progress for a user (stored in KV).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdventureProgress {
    /// User email (from auth).
    #[serde(default)]
    pub user_id: String,
    /// Claim token (if playing from claim flow).
    #[serde(default)]
    pub claim_token: Option<String>,
    /// IDs of completed levels.
    #[serde(default)]
    pub levels_completed: Vec<String>,
    /// All keys ever collected.
    #[serde(default)]
    pub total_keys_collected: Vec<String>,
    /// Per-level scores.
    #[serde(default)]
    pub scores: std::collections::HashMap<String, LevelScore>,
    /// Last played timestamp.
    #[serde(default)]
    pub last_played_at: Option<String>,
}

impl AdventureProgress {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            ..Default::default()
        }
    }

    pub fn is_level_completed(&self, level_id: &str) -> bool {
        self.levels_completed.iter().any(|id| id == level_id)
    }
}
