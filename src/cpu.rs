use std::{collections::HashMap, process::exit, cmp::min};
use super::bus::Bus;

#[derive(Copy, Clone)]
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

enum InstructionSet {
    Arm,
    Thumb
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

pub struct Cpu{
    reg: [u32; 37],
    instr: u32,
    shifter_carry: u32, // 0 or 1 only
    operand1: u32,
    operand2: u32,
    reg_dest: u32,
    actual_pc: u32,

    instr_set: InstructionSet,
    op_mode: OperatingMode, 
    
    reg_map: HashMap<OperatingMode, [Register; 16]>
}

impl Cpu{
    pub fn new() -> Cpu {
        Cpu { 
            reg: [0; 37], 
            instr: 0,
            shifter_carry: 0,
            operand1: 0,
            operand2: 0,
            reg_dest: 0,
            actual_pc: 0,
            
            instr_set: InstructionSet::Arm,
            op_mode: OperatingMode::Usr,

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

    pub fn check_cond(&self) -> bool{
        let cond = self.instr >> 28;
        match cond {
            0b0000 => self.read_flag(Flag::Z),
            0b0001 => !self.read_flag(Flag::Z),
            0b0010 => self.read_flag(Flag::C),
            0b0011 => !self.read_flag(Flag::C),
            0b0100 => self.read_flag(Flag::N),
            0b0101 => !self.read_flag(Flag::N),
            0b0110 => self.read_flag(Flag::V),
            0b0111 => !self.read_flag(Flag::V),
            0b1000 => self.read_flag(Flag::C) && !self.read_flag(Flag::Z),
            0b1001 => !self.read_flag(Flag::C) || self.read_flag(Flag::Z),
            0b1010 => self.read_flag(Flag::N) == !self.read_flag(Flag::V),
            0b1011 => self.read_flag(Flag::N) != self.read_flag(Flag::V),
            0b1100 => !self.read_flag(Flag::Z) && (self.read_flag(Flag::N) == !self.read_flag(Flag::V)),
            0b1101 => self.read_flag(Flag::Z) || (self.read_flag(Flag::N) != self.read_flag(Flag::V)),
            0b1110 => true,
            _ => panic!("cond field not valid")
        }
    }

    // returns extra cycle count. Stores the result into self.operand2. Stores shifter carry into self.shifter_carry.  
    fn process_operand2(&mut self) -> u32 {
        self.shifter_carry = self.read_flag(Flag::C) as u32;
        let is_immediate = (self.instr >> 24) & 1 != 0;
        // immediate value is used
        if is_immediate {
            let cur = (self.instr & 0b11111111) << 24;
            let rotate = (self.instr & 0b111100000000) * 2;
            if rotate > 0{
                self.shifter_carry = (self.instr >> (rotate-1)) & 1;
            }
            self.operand2 = cur.rotate_right(rotate);
            return 0;
        };
        // register is used
        let reg = &self.reg_map.get(&self.op_mode).unwrap()[self.instr as usize & 0b1111];
        let cur = self.reg[*reg as usize];
        let mut shift_amount: u32; 

        let is_immediate = (self.instr >> 4) & 1 == 0;
        // the shift amount is a literal; ie not a register
        if is_immediate {
            shift_amount = (self.instr >> 7) & 0b11111;
        }
        // the shift amount is stored in the lowest byte in a register
        else{
            let reg = (self.instr >> 8) & 0b1111;
            let reg = &self.reg_map.get(&self.op_mode).unwrap()[reg as usize];
            shift_amount = self.reg[*reg as usize] & 0b11111111;
        }

        let shift_type = (self.instr >> 5) & 0b11;
        match shift_type{
            // logical left
            0b00 => {
                if shift_amount > 32 {
                    self.operand2 = 0;
                    self.shifter_carry = 0;
                }
                else{
                    self.operand2 = cur << shift_amount;
                    if shift_amount > 0{
                        self.shifter_carry = (cur >> (32 - shift_amount)) & 1;
                    }
                }
            },
            0b01 => {
                if shift_amount == 0 {
                    shift_amount = 32;
                }
                if shift_amount > 32 {
                    self.operand2 = 0;
                    self.shifter_carry = 0;
                }
                else{
                    self.operand2 = cur >> shift_amount;
                    if shift_amount > 0{
                        self.shifter_carry = (cur >> (shift_amount-1)) & 1;
                    }
                }
            },
            0b10 => {
                {
                    if shift_amount == 0 {
                        shift_amount = 32;
                    }
                    shift_amount = min(shift_amount, 32);
                    
                    self.operand2 = cur >> shift_amount;
                    if shift_amount > 0 {
                        self.shifter_carry = (cur >> (shift_amount-1)) & 1;
                    }
                    
                    if cur >> 31 & 1 > 0 {
                        self.operand2 |= (0xffffffff >> (32 - shift_amount)) << (32 - shift_amount);
                    }
                }
            },
            0b11 => {
                if shift_amount > 0{
                    self.shifter_carry = (self.instr >> (shift_amount % 32-1)) & 1;
                    self.operand2 = cur.rotate_right(shift_amount);
                }
                else{
                    self.shifter_carry = self.instr & 1;
                    self.operand2 = (cur >> 1) | ((self.read_flag(Flag::C) as u32) << 31)
                }
            },
            _ => {}
        }

        return !is_immediate as u32;
    }

    fn process_operand1(&mut self) -> u32 {
        let reg = (self.instr >> 16) & 0b1111;
        self.operand1 = self.read_reg(reg);
        0
    }

    fn process_reg_dest(&mut self) -> u32 {
        self.reg_dest = (self.instr >> 12) & 0b1111;
        0
    }

    //TODO: note copy to CPSR when dest is R15

    fn op_adc(&mut self) -> u32 {
        let res = self.operand1 + self.operand2 + self.read_flag(Flag::C) as u32;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, (self.operand1 >> 31 > 0 || self.operand2 >> 31 > 0) && res >> 31 == 0);
            self.set_flag(Flag::V, (self.operand1 >> 31 == self.operand2 >> 31) && res >> 31 != self.operand1 >> 31);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_add(&mut self) -> u32 {
        let res = self.operand1 + self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, (self.operand1 >> 31 > 0 || self.operand2 >> 31 > 0) && res >> 31 == 0);
            self.set_flag(Flag::V, (self.operand1 >> 31 == self.operand2 >> 31) && res >> 31 != self.operand1 >> 31);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_and(&mut self) -> u32 {
        let res = self.operand1 & self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_bic(&mut self) -> u32 {
        let res = self.operand1 & !self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_cmn(&mut self) -> u32 {
        let res = self.operand1 + self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, (self.operand1 >> 31 > 0 || self.operand2 >> 31 > 0) && res >> 31 == 0);
            self.set_flag(Flag::V, (self.operand1 >> 31 == self.operand2 >> 31) && res >> 31 != self.operand1 >> 31);
        }
        0
    }

    fn op_cmp(&mut self) -> u32 {
        let res = self.operand1 - self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, !(self.operand2 > self.operand1));
            self.set_flag(Flag::V, (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand2 >> 31);
        }
        0
    }

    fn op_eor(&mut self) -> u32 {
        let res = self.operand1 ^ self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_mov(&mut self) -> u32 {
        self.reg[self.reg_dest as usize] = self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, self.operand2 >> 31 > 0);
            self.set_flag(Flag::Z, self.operand2 == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_mvn(&mut self) -> u32 {
        let res = !self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_orr(&mut self) -> u32 {
        let res = self.operand1 | self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_rsb(&mut self) -> u32 {
        let res = self.operand2 - self.operand1;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, !(self.operand1 > self.operand2));
            self.set_flag(Flag::V, (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand1 >> 31);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_rsc(&mut self) -> u32 {
        let flag_c = self.read_flag(Flag::C);
        let res = self.operand2 - self.operand1 + flag_c as u32 - 1;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, if self.operand1 > self.operand2 {false} else {true});
            
            let overflow = (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand1 >> 31;
            if flag_c {
                self.set_flag(Flag::V, overflow);
            }
            else{
                self.set_flag(Flag::V, (!overflow && res == 0) || (overflow && res > 0));
            }
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_sbc(&mut self) -> u32 {
        let flag_c = self.read_flag(Flag::C);
        let res = self.operand1 - self.operand2 + flag_c as u32 - 1;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, if self.operand1 > self.operand2 {false} else {true});
            
            let overflow = (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand2 >> 31;
            if flag_c {
                self.set_flag(Flag::V, overflow);
            }
            else{
                self.set_flag(Flag::V, (!overflow && res == 0) || (overflow && res > 0));
            }
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_sub(&mut self) -> u32 {
        let res = self.operand1 - self.operand2;
        self.reg[self.reg_dest as usize] = res;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, !(self.operand1 > self.operand2));
            self.set_flag(Flag::V, (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand2 >> 31);
        }
        (self.reg_dest == Register::R15 as u32) as u32
    }

    fn op_teq(&mut self) -> u32 {
        let res = self.operand1 ^ self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        0
    }

    fn op_tst(&mut self) -> u32 {
        let res = self.operand1 & self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        0
    }

    pub fn clock(&mut self, bus: &mut Bus) {
        self.instr = bus.read_word(self.actual_pc as usize);
        self.set_pc(self.actual_pc + 8);
        
        let cur_cycles = match (self.instr >> 24) & 0b111 {
            0b000 => self.execute_dataproc(),
            _ => {
                print!("Error undefined instruction {:#034b} at pc {}\n", self.instr, self.actual_pc);
                0
            }
        };
    }

    // returns number of clock cycles
    pub fn execute_dataproc(&mut self) -> u32 {
        let mut cur_cycles = self.process_reg_dest() + self.process_operand1() + self.process_operand2();

        cur_cycles += match (self.instr >> 21) & 0b1111 {
            0b0000 => self.op_and(),
            0b0001 => self.op_eor(),
            0b0010 => self.op_sub(),
            0b0011 => self.op_rsb(),
            0b0100 => self.op_add(),
            0b0101 => self.op_adc(),
            0b0110 => self.op_sbc(),
            0b0111 => self.op_rsc(),
            0b1000 => self.op_tst(),
            0b1001 => self.op_teq(),
            0b1010 => self.op_cmp(),
            0b1011 => self.op_cmn(),
            0b1100 => self.op_orr(),
            0b1101 => self.op_mov(),
            0b1110 => self.op_bic(),
            0b1111 => self.op_mvn(),
            _ => {
                print!("Error undefined instruction {:#034b} at pc {}, data processing opcode unknown\n", self.instr, self.actual_pc);
                0
            }
        };

        cur_cycles
    }

    fn dataproc_set_cond(&self) -> bool{
        (self.instr >> 20) & 1 > 0
    }

    pub fn read_pc(&self) -> u32 {
        self.reg[Register::R15 as usize]
    }

    pub fn set_pc(&mut self, pc: u32){
        self.reg[Register::R15 as usize] = pc;
    }

    pub fn read_sp(&self) -> u32 {
        self.reg[Register::R14 as usize]
    }

    pub fn set_sp(&mut self, sp: u32){
        self.reg[Register::R14 as usize] = sp;
    }

    pub fn read_flag(&self, f: Flag) -> bool {
        let s = f as u32;
        (self.reg[Register::CPSR as usize] << s) != 0
    }

    pub fn set_flag(&mut self, f: Flag, val: bool) {
        let s = f as u32;
        self.reg[Register::CPSR as usize] |= (val as u32) << s;
    }

    fn read_reg(&self, reg: u32) -> u32 {
        let reg = &self.reg_map.get(&self.op_mode).unwrap()[reg as usize];
        self.reg[*reg as usize]
    }

    fn set_reg(&mut self, reg: u32, val: u32) {
        let reg = &self.reg_map.get(&self.op_mode).unwrap()[reg as usize];
        self.reg[*reg as usize] = val;
    }
}