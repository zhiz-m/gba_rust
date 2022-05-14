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

#[derive(PartialEq)]
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
    
    reg_map: HashMap<OperatingMode, [Register; 16]>,
    spsr_map: HashMap<OperatingMode, Register>,
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
            ]),
            spsr_map: HashMap::from([
                (OperatingMode::Fiq, Register::SPSR_fiq),
                (OperatingMode::Svc, Register::SPSR_svc),
                (OperatingMode::Abt, Register::SPSR_abt),
                (OperatingMode::Irq, Register::SPSR_irq),
                (OperatingMode::Und, Register::SPSR_und),
            ]),
        }
    }

    // ---------- main loop (clock)

    pub fn clock(&mut self, bus: &mut Bus) {
        // get rid of the trailing bits, these may be set to 1 but must always be treated as 0
        let aligned_pc = match self.instr_set {
            InstructionSet::Arm => self.actual_pc & !11,
            InstructionSet::Thumb => self.actual_pc &!1, 
        };
        self.instr = bus.read_word(aligned_pc as usize);
        self.set_pc(self.actual_pc + 8);

        let mut cur_cycles = 0;
        
        let mut increment_pc = true;

        if self.check_cond() {
            cur_cycles +=
            // branch and exchange shares 0b000 with execute_dataproc. 
            if (self.instr << 4) >> 8 == 0b000100101111111111110001{
                increment_pc = false;
                self.execute_branch_exchange()
            }
            // load and store instructions
            // swp: note that this must be checked before execute_ldr_str and execute_halfword_signed_transfer
            else if (self.instr >> 23) & 0b11111 == 0b00010 && (self.instr >> 20) & 0b11 == 0 && (self.instr >> 4) & 0b11111111 == 0b1001 {
                self.execute_swp(bus)
            }
            else if (self.instr >> 26) & 0b11 == 1 {
                self.execute_ldr_str(bus)
            }
            else if (self.instr >> 25) & 0b111 == 0 && 
                ((self.instr >> 22) & 1 == 0 && (self.instr >> 7) & 0b11111 == 1 && (self.instr >> 4) & 1 == 1) ||
                ((self.instr >> 22) & 1 == 1 && (self.instr >> 7) & 1 == 1 && (self.instr >> 4) & 1 == 1) {
                self.execute_halfword_signed_transfer(bus)
            }
            // msr and mrs
            else if (self.instr >> 23) & 0b11111 == 0b00010 && (self.instr >> 16) & 0b111111 == 0b001111 && self.instr & 0b111111111111 == 0{
                self.execute_mrs_psr2reg()
            } 
            else if (self.instr >> 23) & 0b11111 == 0b00010 && (self.instr >> 12) & 0b1111111111 == 0b1010011111 && (self.instr >> 4) & 0b1111111111 == 0{
                self.execute_msr_reg2psr()
            } 
            else if (self.instr >> 26) & 0b11 == 0 && (self.instr >> 23) & 0b11 == 0b10 && (self.instr >> 12) & 0b1111111111 == 0b1010001111{
                self.execute_msr_reg2psr()
            } 
            // multiply and multiply_long share 0b000 with execute_dataproc. 
            else if (self.instr >> 22) & 0b111111 == 0 && (self.instr >> 4) & 0b1111 == 0b1001{
                self.execute_multiply()
            }
            else if (self.instr >> 23) & 0b11111 == 1 && (self.instr >> 4) & 0b1111 == 0b1001{
                self.execute_multiply_long()
            }
            else{
                match (self.instr >> 24) & 0b111 {
                    0b000 | 0b001 => self.execute_dataproc(),
                    0b101 => {
                        increment_pc = false;
                        self.execute_branch()
                    },
                    _ => {
                        print!("Error undefined instruction {:#034b} at pc {}\n", self.instr, self.actual_pc);
                        0
                    }
                }
            }
        }

        if increment_pc {
            self.actual_pc += match self.instr_set {
                InstructionSet::Arm => 0b100,
                InstructionSet::Thumb => 0b010,
            }
        }
    }

    // ---------- branches
    fn execute_branch(&mut self) -> u32 {
        // link bit set
        if (self.instr >> 24) & 1 == 1 {
            self.reg[Register::R14 as usize] = self.actual_pc + 4;
        }
        let mut offset = (self.instr << 10) >> 8;
        if (offset >> 25) & 1 == 1 {
            offset |= 0b111111 << 26;
        }
        self.actual_pc = self.read_pc() + offset;
        3
    }

    fn execute_branch_exchange(&mut self) -> u32 {
        assert!(self.instr_set == InstructionSet::Arm);
        let addr = self.read_reg(self.instr & 0b1111);
        if addr & 1 > 0 {
            self.instr_set = InstructionSet::Thumb;
        };
        self.actual_pc = addr;
        3
    }

    // ---------- data processing

    // returns number of clock cycles
    fn execute_dataproc(&mut self) -> u32 {
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
    // ---------- MRS and MSR
    fn execute_mrs_psr2reg(&mut self) -> u32 {
        let reg = if (self.instr >> 22 & 1) == 0 {Register::CPSR} else {*self.spsr_map.get(&self.op_mode).unwrap()};
        let res = self.reg[reg as usize];
        self.reg_dest = (self.instr >> 12) & 0b1111;
        self.set_reg(self.reg_dest, res);
        1
    }

    // NOTE: inconsistencies between ARM7TDMI_data_sheet.pdf and cpu_technical_spec_long.pdf regarding MSR. 
    // ARM7TDMI_data_sheet.pdf was chosen as the source of truth. TODO: check if this is the correct choice. 
    fn execute_msr_reg2psr(&mut self) -> u32 {
        let reg_dest = if (self.instr >> 22 & 1) == 0 {Register::CPSR} else {*self.spsr_map.get(&self.op_mode).unwrap()};
        let res = self.read_reg(self.instr & 0b1111);
        self.reg[reg_dest as usize] = res;
        1
    }

    fn execute_msr_reg_imm2psr(&mut self) -> u32 {
        let reg_dest = if (self.instr >> 22 & 1) == 0 {Register::CPSR} else {*self.spsr_map.get(&self.op_mode).unwrap()};
        let res = if (self.instr >> 25) & 1 == 0 { // register
            self.read_reg(self.instr & 0b1111)
        }
        else{ // immediate
            self.process_immediate_rotate();
            self.operand2
        };
        self.reg[reg_dest as usize] = res;
        1
    }

    // ---------- multiplications
    fn execute_multiply(&mut self) -> u32 {
        self.reg_dest = (self.instr >> 16) & 0b1111;
        self.operand1 = self.read_reg((self.instr >> 12) & 0b1111);
        self.operand2 = self.read_reg((self.instr >> 8) & 0b1111);
        let operand3 = self.read_reg((self.instr) & 0b1111);

        let mut cur_cycles;

        let res = if (self.instr >> 21) & 1 > 0 {
            cur_cycles = 2;
            operand3 * self.operand2 + self.operand1
        }
        else {
            cur_cycles = 1;
            operand3 * self.operand2
        };

        self.set_reg(self.reg_dest, res);

        if (self.instr >> 20) & 1 == 1{
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
        }

        cur_cycles += 
        if self.operand2 >> 8 == 0 || self.operand2 >> 8 == (1 << 24) - 1{
            1
        }
        else if self.operand2 >> 16 == 0 || self.operand2 >> 16 == (1 << 16) - 1{
            2
        }
        else if self.operand2 >> 24 == 0 || self.operand2 >> 24 == (1 << 8) - 1{
            3
        }
        else{
            4
        };

        cur_cycles
    }

    fn execute_multiply_long(&mut self) -> u32{
        let reg_dest_hi = (self.instr >> 16) & 0b1111;
        let reg_dest_lo = (self.instr >> 12) & 0b1111;
        let operand2 = self.read_reg((self.instr >> 8) & 0b1111);
        let operand3 = self.read_reg((self.instr) & 0b1111);
        let operand1 = (self.read_reg(reg_dest_hi) as u64) << 32 + self.read_reg(reg_dest_lo);

        let mut cur_cycles;
        let unsigned = (self.instr >> 22) & 1 == 0;

        let res = if (self.instr >> 21) & 1 > 0 {
            cur_cycles = 2;
            if unsigned{
                operand1 + operand3 as u64 * operand2 as u64
            }
            else{
                (operand1 as i64 + operand3 as i64 * operand2 as i64) as u64
            }
        }
        else {
            cur_cycles = 1;
            if unsigned{
                operand3 as u64 * operand2 as u64
            }
            else{
                (operand3 as i64 * operand2 as i64) as u64
            }
        };

        self.set_reg(reg_dest_hi, (res >> 32) as u32);
        self.set_reg(reg_dest_lo, res as u32);

        if (self.instr >> 20) & 1 == 1{
            self.set_flag(Flag::N, res >> 63 > 0);
            self.set_flag(Flag::Z, res == 0);
        }

        cur_cycles += 
        if operand2 >> 8 == 0 || (!unsigned && operand2 >> 8 == (1 << 24) - 1){
            1
        }
        else if operand2 >> 16 == 0 || (!unsigned && operand2 >> 16 == (1 << 16) - 1){
            2
        }
        else if operand2 >> 24 == 0 || (!unsigned && operand2 >> 24 == (1 << 8) - 1){
            3
        }
        else{
            4
        };

        cur_cycles
    }

    // ---------- data transfers
    fn execute_ldr_str(&mut self, bus: &mut Bus) -> u32 {
        let mut cycles = 0;
        // I flag
        let offset = if (self.instr >> 25) & 1 > 0 {
            // NOTE: double check if cycles are added here
            //cycles += 
            self.process_reg_rotate();
            self.operand2
        }
        else{
            self.instr & 0b111111111111
        };
        // base reg
        let base_reg = (self.instr >> 16) & 0b1111;
        let mut addr =  self.read_reg(base_reg);

        // U flag
        let offset_addr = if (self.instr >> 23) & 1 == 0{
            addr - offset
        }
        else{
            addr + offset
        };

        // P flag
        if (self.instr >> 24) == 1{
            addr = offset_addr;
        }

        let addr = addr as usize;

        let L = (self.instr >> 20) & 1 == 1;
        let B = (self.instr >> 22) & 1 == 1;

        let reg = (self.instr >> 12) & 0b1111;

        match (L,B) {
            // register -> memory, byte
            (false, true) => {
                bus.store_byte(addr, (self.read_reg(reg) + if reg == Register::R15 as u32 {4} else {0}) as u8);
                cycles += 2;
            },
            // register -> memory, word
            (false, false) => {
                let res = self.read_reg(reg) + if reg == Register::R15 as u32 {4} else {0};
                if (addr & 1) == 1{
                    bus.store_byte(addr, res as u8);
                    bus.store_byte(addr + 1, (res >> 8) as u8);
                    bus.store_byte(addr + 2, (res >> 16) as u8);
                    bus.store_byte(addr + 3, (res >> 24) as u8);
                }
                else if (addr & 0b10) > 0 {
                    bus.store_halfword(addr, res as u16);
                    bus.store_halfword(addr + 2, (res >> 16) as u16);
                }
                else{
                    bus.store_word(addr, res);
                }
                cycles += 2;
            },
            // memory -> register, byte
            (true, true) => {
                let res = bus.read_byte(addr);
                self.set_reg(reg, res as u32);
                cycles += 3;
            },
            // memory -> register, word
            (true, false) => {
                if (addr & 0b10) > 0 {
                    let hi = bus.read_halfword(addr) as u32;
                    let lo = bus.read_halfword(addr + 2) as u32;
                    self.set_reg(reg, lo + (hi << 16));
                }
                else{
                    let res = bus.read_word(addr);
                    self.set_reg(reg, res);
                }
                cycles += 3;
            },
        };

        // W flag
        if (self.instr >> 21) == 1 {
            self.set_reg(base_reg, offset_addr);
        };

        if L && reg == Register::R15 as u32 {
            cycles += 2;
        }

        cycles
    }

    fn execute_halfword_signed_transfer(&mut self, bus: &mut Bus) -> u32 {
        let mut cycles = 0;
        let offset = if (self.instr >> 22) & 1 == 0 {
            self.read_reg(self.instr & 0b1111)
        }
        else{
            let hi = (self.instr >> 8) & 0b1111;
            let lo = self.instr & 0b1111;
            lo + hi << 4
        };
        // base reg
        let base_reg = (self.instr >> 16) & 0b1111;
        let mut addr =  self.read_reg(base_reg);

        // U flag
        let offset_addr = if (self.instr >> 23) & 1 == 0{
            addr - offset
        }
        else{
            addr + offset
        };

        // P flag
        if (self.instr >> 24) == 1{
            addr = offset_addr;
        }

        let addr = addr as usize;

        let L = (self.instr >> 20) & 1 == 1;
        let S = (self.instr >> 6) & 1 == 1;
        let H = (self.instr >> 5) & 1 == 1;

        let reg = (self.instr >> 12) & 0b1111;

        match (L,S,H) {
            // register -> memory, byte (STRH)
            (false, false, true) => {
                let res = self.read_reg(reg);
                bus.store_halfword(addr, res as u16);
            },
            // LDRH
            (true, false, true) => {
                self.set_reg(reg, bus.read_halfword(addr) as u32);
            },
            // LDRSH
            (true, true, true) => {
                let mut res = bus.read_halfword(addr) as u32;
                if (res >> 15) & 1 > 0{
                    res |= ((1<<16) - 1) << 16;
                }
                self.set_reg(reg, res);
            },
            // LDRSB
            (true, true, false) => {
                let mut res = bus.read_byte(addr) as u32;
                if (res >> 7) & 1 > 0{
                    res |= ((1<<24) - 1) << 8;
                }
                self.set_reg(reg, res);
            },
            _ => {
                panic!("Error undefined combination in execute_halfword_signed_transfer with instr {:#034b} at pc {}\n", self.instr, self.actual_pc);
            }
        };

        // W flag
        if (self.instr >> 21) == 1 {
            self.set_reg(base_reg, offset_addr);
        };

        if (L,S,H) == (false, false, true) {
            2
        }
        else if reg == Register::R15 as u32 {
            5
        }
        else {
            3
        }
    }

    fn execute_swp(&mut self, bus: &mut Bus) -> u32 {
        0
    }

    // ---------- miscellaneous helpers

    fn check_cond(&self) -> bool{
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

    fn dataproc_set_cond(&self) -> bool{
        (self.instr >> 20) & 1 > 0
    }

    // modifies self.operand2. returns number of extra cycles (0)
    fn process_immediate_rotate(&mut self) -> u32 {
        let cur = (self.instr & 0b11111111) << 24;
        let rotate = ((self.instr >> 8) & 0b1111) * 2;
        if rotate > 0{
            self.shifter_carry = (self.instr >> (rotate-1)) & 1;
        }
        self.operand2 = cur.rotate_right(rotate);
        0
    }

    fn process_reg_rotate(&mut self) -> u32 {
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

    // returns extra cycle count. Stores the result into self.operand2. Stores shifter carry into self.shifter_carry.  
    fn process_operand2(&mut self) -> u32 {
        self.shifter_carry = self.read_flag(Flag::C) as u32;
        let is_immediate = (self.instr >> 24) & 1 != 0;
        // immediate value is used
        if is_immediate {
            self.process_immediate_rotate()
        }
        else{
            self.process_reg_rotate()
        }
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

    // ---------- read and set helpers

    fn read_pc(&self) -> u32 {
        self.reg[Register::R15 as usize]
    }

    fn set_pc(&mut self, pc: u32){
        self.reg[Register::R15 as usize] = pc;
    }

    fn read_sp(&self) -> u32 {
        self.reg[Register::R14 as usize]
    }

    fn set_sp(&mut self, sp: u32){
        self.reg[Register::R14 as usize] = sp;
    }

    fn read_flag(&self, f: Flag) -> bool {
        let s = f as u32;
        (self.reg[Register::CPSR as usize] << s) != 0
    }

    fn set_flag(&mut self, f: Flag, val: bool) {
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