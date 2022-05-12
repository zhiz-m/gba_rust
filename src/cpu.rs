use std::{collections::HashMap, process::exit};

enum Register{
    R0, R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, 
    CPSR, R8_fiq, R9_fiq, R10_fiq, R11_fiq, R12_fiq, 
    R13_fiq, R13_svc, R13_abt, R13_irq, R13_und, 
    R14_fiq, R14_svc, R14_abt, R14_irq, R14_und, 
    SPSR_fiq, SPSR_svc, SPSR_abt, SPSR_irq, SPSR_und
}

#[derive(Hash, PartialEq, Eq)]
enum OperatingMode{
    Usr,
    Fiq, 
    Irq,
    Svc,
    Abt, 
    Sys, 
    Und
}

enum Flag{
    N = 31,
    Z = 30,
    C = 29,
    V = 28,
    I = 7,
    F = 6,
    T = 5,
}

struct Cpu{
    reg: [u32; 37],
    instr: u32,
    
    reg_map: HashMap<OperatingMode, [Register; 16]>
}

impl Cpu{
    pub fn new() -> Cpu {
        Cpu { 
            reg: [0; 37], 
            instr: 0,

            reg_map: HashMap::from([
                (OperatingMode::Usr, [Register::R0, Register::R1, Register::R2, Register::R3, Register::R4, Register::R5, Register::R6, Register::R7, Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13, Register::R14, Register::R15]),
                (OperatingMode::Fiq, [Register::R0, Register::R1, Register::R2, Register::R3, Register::R4, Register::R5, Register::R6, Register::R7, Register::R8_fiq, Register::R9_fiq, Register::R10_fiq, Register::R11_fiq, Register::R12_fiq, Register::R13_fiq, Register::R14_fiq, Register::R15]),
                (OperatingMode::Svc, [Register::R0, Register::R1, Register::R2, Register::R3, Register::R4, Register::R5, Register::R6, Register::R7, Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13_svc, Register::R14_svc, Register::R15]),
                (OperatingMode::Abt, [Register::R0, Register::R1, Register::R2, Register::R3, Register::R4, Register::R5, Register::R6, Register::R7, Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13_abt, Register::R14_abt, Register::R15]),
                (OperatingMode::Irq, [Register::R0, Register::R1, Register::R2, Register::R3, Register::R4, Register::R5, Register::R6, Register::R7, Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13_irq, Register::R14_irq, Register::R15]),
                (OperatingMode::Und, [Register::R0, Register::R1, Register::R2, Register::R3, Register::R4, Register::R5, Register::R6, Register::R7, Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13_und, Register::R14_und, Register::R15]),
            ])
        }
    }

    pub fn checkCond(&self) -> bool{
        let cond = self.instr >> 28;
        match cond {
            0 => self.get_flag(Flag::Z),
            1 => !self.get_flag(Flag::Z),
            2 => self.get_flag(Flag::C),
            3 => !self.get_flag(Flag::C),
            4 => self.get_flag(Flag::N),
            5 => !self.get_flag(Flag::N),
            6 => self.get_flag(Flag::V),
            7 => !self.get_flag(Flag::V),
            8 => self.get_flag(Flag::C) && !self.get_flag(Flag::Z),
            9 => !self.get_flag(Flag::C) || self.get_flag(Flag::Z),
            10 => self.get_flag(Flag::N) == !self.get_flag(Flag::V),
            11 => self.get_flag(Flag::N) != self.get_flag(Flag::V),
            12 => !self.get_flag(Flag::Z) && (self.get_flag(Flag::N) == !self.get_flag(Flag::V)),
            13 => self.get_flag(Flag::Z) || (self.get_flag(Flag::N) != self.get_flag(Flag::V)),
            14 => true,
            _ => panic!("cond field not valid")
        }
    }

    pub fn get_pc(&self) -> u32 {
        self.reg[Register::R15 as usize]
    }

    pub fn set_pc(&mut self, pc: u32){
        self.reg[Register::R15 as usize] = pc;
    }

    pub fn get_sp(&self) -> u32 {
        self.reg[Register::R14 as usize]
    }

    pub fn set_sp(&mut self, sp: u32){
        self.reg[Register::R14 as usize] = sp;
    }

    pub fn get_flag(&self, f: Flag) -> bool {
        let s = f as u32;
        (self.reg[Register::CPSR as usize] << s) != 0
    }

    pub fn set_flag(&mut self, f: Flag, val: bool) {
        let s = f as u32;
        self.reg[Register::CPSR as usize] |= (val as u32) << s;
    }
}