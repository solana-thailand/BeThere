//! Test level and levels 1–5 (basics).


use crate::pages::adventure::types::*;

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
                dialog: "if expressions in Rust work like other languages, but they're expressions — they return a value! No ternary operator needed.".to_string(),
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
                text: "match is Rust's pattern matching powerhouse — like a super-powered switch statement.".to_string(),
            },
            SignDef {
                pos: (10, 8),
                text: "for loops iterate over anything that implements IntoIterator — vectors, ranges, strings!".to_string(),
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
