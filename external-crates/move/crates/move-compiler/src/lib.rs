// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

// #![forbid(unsafe_code)]

#[macro_use(sp)]
extern crate move_ir_types;

#[macro_use(symbol)]
extern crate move_symbol_pool;

pub mod cfgir;
pub mod command_line;
pub mod compiled_unit;
pub mod diagnostics;
pub mod editions;
pub mod expansion;
pub mod hlir;
pub mod interface_generator;
pub mod ir_translation;
pub mod linters;
pub mod naming;
pub mod parser;
pub mod shared;
pub mod sui_mode;
mod to_bytecode;
pub mod typing;
pub mod unit_test;

pub use command_line::{
    MOVE_COMPILED_INTERFACES_DIR,
    compiler::{
        Compiler, FullyCompiledProgram, PASS_CFGIR, PASS_COMPILATION, PASS_EXPANSION, PASS_HLIR,
        PASS_NAMING, PASS_PARSER, PASS_TYPING, SteppedCompiler, construct_pre_compiled_lib,
        generate_interface_files, output_compiled_units,
    },
};
pub use shared::Flags;
