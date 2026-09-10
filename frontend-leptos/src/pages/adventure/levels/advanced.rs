//! Levels 6–10 (ownership through traits).

use crate::pages::adventure::types::*;

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
