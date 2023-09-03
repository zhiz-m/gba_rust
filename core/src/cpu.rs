#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use log::warn;

use crate::{
    bus::{Bus, MemoryRegion},
    config,
    dma_channel::DMA_Channel,
};
use std::{cmp::min, collections::{HashMap, VecDeque}, num::Wrapping};

#[derive(Copy, Clone, PartialEq)]
enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    Cpsr,
    R8_fiq,
    R9_fiq,
    R10_fiq,
    R11_fiq,
    R12_fiq,
    R13_fiq,
    R13_svc,
    R13_abt,
    R13_irq,
    R13_und,
    R14_fiq,
    R14_svc,
    R14_abt,
    R14_irq,
    R14_und,
    SPSR_fiq,
    SPSR_svc,
    SPSR_abt,
    SPSR_irq,
    SPSR_und,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
enum OperatingMode {
    Usr = 0,
    Fiq = 1,
    Irq = 2,
    Svc = 3,
    Abt = 4,
    Sys = 5,
    Und = 6,
}

#[derive(PartialEq, Eq)]
pub enum Flag {
    N = 31,
    Z = 30,
    C = 29,
    V = 28,
    I = 7,
    //F = 6,
    T = 5,
}

pub struct Cpu {
    //arm_instr_table: Vec<fn(&mut Cpu, &mut Bus) -> u32>,
    reg: [u32; 37],
    pub instr: u32,
    shifter_carry: u32, // 0 or 1 only
    operand1: u32,
    operand2: u32,
    reg_dest: u32,
    pub actual_pc: u32,
    pub pipeline_instr: VecDeque<u32>,

    op_mode: OperatingMode,

    reg_map: [[Register; 16]; 7],
    spsr_map: HashMap<OperatingMode, Register>,

    increment_pc: bool,
    thumb_modify_flags: bool,

    halt: bool,
    pub interrupt_requested: bool,
    //interrupt: u16, // same format as REG_IE and REG_IF. But, it is cleared to 0 everytime an interrupt begins executing to prevent infinite loop.
    #[cfg(feature = "debug_instr")]
    pub debug_cnt: u32,
    #[cfg(feature = "debug_instr")]
    bios_end: bool,

    pub last_fetched_bios_instr: u32,
}

impl Cpu {
    pub fn new() -> Cpu {
        let mut res = Cpu {
            //arm_instr_table: Cpu::generate_arm_decode_table(),
            reg: [0; 37],
            instr: 0,
            shifter_carry: 0,
            operand1: 0,
            operand2: 0,
            reg_dest: 0,
            //actual_pc: 0x08000000,
            //actual_pc: 0x080002f0,
            actual_pc: 0,
            pipeline_instr: VecDeque::<u32>::with_capacity(3),

            op_mode: OperatingMode::Sys,

            reg_map: [
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8,
                    Register::R9,
                    Register::R10,
                    Register::R11,
                    Register::R12,
                    Register::R13,
                    Register::R14,
                    Register::R15,
                ],
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8_fiq,
                    Register::R9_fiq,
                    Register::R10_fiq,
                    Register::R11_fiq,
                    Register::R12_fiq,
                    Register::R13_fiq,
                    Register::R14_fiq,
                    Register::R15,
                ],
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8,
                    Register::R9,
                    Register::R10,
                    Register::R11,
                    Register::R12,
                    Register::R13_irq,
                    Register::R14_irq,
                    Register::R15,
                ],
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8,
                    Register::R9,
                    Register::R10,
                    Register::R11,
                    Register::R12,
                    Register::R13_svc,
                    Register::R14_svc,
                    Register::R15,
                ],
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8,
                    Register::R9,
                    Register::R10,
                    Register::R11,
                    Register::R12,
                    Register::R13_abt,
                    Register::R14_abt,
                    Register::R15,
                ],
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8,
                    Register::R9,
                    Register::R10,
                    Register::R11,
                    Register::R12,
                    Register::R13,
                    Register::R14,
                    Register::R15,
                ],
                [
                    Register::R0,
                    Register::R1,
                    Register::R2,
                    Register::R3,
                    Register::R4,
                    Register::R5,
                    Register::R6,
                    Register::R7,
                    Register::R8,
                    Register::R9,
                    Register::R10,
                    Register::R11,
                    Register::R12,
                    Register::R13_und,
                    Register::R14_und,
                    Register::R15,
                ],
            ],
            spsr_map: HashMap::from([
                (OperatingMode::Fiq, Register::SPSR_fiq),
                (OperatingMode::Svc, Register::SPSR_svc),
                (OperatingMode::Abt, Register::SPSR_abt),
                (OperatingMode::Irq, Register::SPSR_irq),
                (OperatingMode::Und, Register::SPSR_und),
            ]),

            increment_pc: true,
            thumb_modify_flags: true,

            halt: false,
            interrupt_requested: false,

            #[cfg(feature = "debug_instr")]
            debug_cnt: 0,
            #[cfg(feature = "debug_instr")]
            bios_end: false,

            last_fetched_bios_instr: 0,
        };
        //res.set_reg(13, 0x03007F00);
        //res.reg[Register::R13_svc as usize] = 0x02FFFFF0;

        // set CPSR for sys mode
        res.set_cpsr(0b11111);
        res
    }

    // ---------- main loop (clock)
    #[inline(always)]
    pub fn clock(&mut self, bus: &mut Bus) -> u32 {
        //use std::iter::repeat;

        #[cfg(feature = "debug_instr")]
        if self.actual_pc >= 0x8000000 {
            self.bios_end = true;
        }
        // check for halting (pause cpu)
        /*if bus.check_cpu_halt_request() {
            self.halt();
        }*/

        //self.debug(&format!("halting: {}\n", self.halt));
        //self.debug(&format!("IE: {:#018b}\n", bus.read_halfword(0x04000200)));

        let clocks = if !self.read_flag(Flag::I) && self.interrupt_requested {
            self.halt = false;
            //self.bus_set_reg_if(bus);
            //info!("interrupt: {:#018b}", bus.read_halfword(0x04000200));
            //self.debug = true;
            self.execute_hardware_interrupt()
        } else if self.check_dma(bus) {
            self.execute_dma(bus)
        } else if self.halt {
            config::CPU_HALT_SLEEP_CYCLES // consume clock cycles; do nothing
        } else {
            match self.read_flag(Flag::T) {
                false => self.decode_execute_instruction_arm(bus),
                true => self.decode_execute_instruction_thumb(bus),
            }
        };

        assert!(clocks > 0);
        clocks
    }

    // -------------- ARM INSTRUCTIONS -----------------

    #[inline(always)]
    fn fetch_arm_instr(&mut self, bus: &mut Bus){
        if self.pipeline_instr.is_empty(){
            self.pipeline_instr.push_back(bus.read_word(self.actual_pc as usize));
            self.pipeline_instr.push_back(bus.read_word(self.actual_pc as usize + 4));
        }
        self.pipeline_instr.push_back(bus.read_word(self.actual_pc as usize + 8));
        self.instr = self.pipeline_instr.pop_front().unwrap();
        if self.actual_pc < 0x4000 {
            self.last_fetched_bios_instr =
                bus.read_word_raw(self.actual_pc as usize + 8, MemoryRegion::Bios) as u32;
        }
    }


    // completes one instruction. Returns number of clock cycles
    #[inline(always)]
    fn decode_execute_instruction_arm(&mut self, bus: &mut Bus) -> u32 {
        // get rid of the trailing bits, these may be set to 1 but must always be treated as 0
        self.actual_pc &= !0b11;
        self.fetch_arm_instr(bus);
        self.set_pc(self.actual_pc + 8);

        //if self.actual_pc == 0x80002f0  {
        //    info!("   reached");
        //}

        let mut cur_cycles = 0;

        self.increment_pc = true;

        #[cfg(feature = "debug_instr")]
        self.print_pc(bus);

        if self.check_cond(self.instr >> 28) {
            cur_cycles += if (self.instr << 4) >> 8 == 0b000100101111111111110001 {
                // branch and exchange shares 0b000 with execute_dataproc.
                #[cfg(feature = "debug_instr")]
                self.debug("        BX");
                self.execute_branch_exchange()
            } else if (self.instr >> 24) & 0b1111 == 0b1111 {
                // software interrupt
                #[cfg(feature = "debug_instr")]
                self.debug("        SWI");
                self.execute_software_interrupt()
            } else if (self.instr >> 22) & 0b111111 == 0 && (self.instr >> 4) & 0b1111 == 0b1001 {
                // multiply and multiply_long share 0b000 with execute_dataproc.
                #[cfg(feature = "debug_instr")]
                self.debug("        MUL, MLA");
                self.execute_multiply()
            } else if (self.instr >> 23) & 0b11111 == 1 && (self.instr >> 4) & 0b1111 == 0b1001 {
                #[cfg(feature = "debug_instr")]
                self.debug("        multiply long");
                self.execute_multiply_long()
            } else if (self.instr >> 23) & 0b11111 == 0b00010
                && (self.instr >> 20) & 0b11 == 0
                && (self.instr >> 4) & 0b11111111 == 0b1001
            {
                // load and store instructions
                // swp: note that this must be checked before execute_ldr_str and execute_halfword_signed_transfer
                #[cfg(feature = "debug_instr")]
                self.debug("        SWP");
                self.execute_swp(bus)
            } else if (self.instr >> 26) & 0b11 == 1 {
                #[cfg(feature = "debug_instr")]
                self.debug("        LDR, STR");
                self.execute_ldr_str(bus)
            } else if (self.instr >> 25) & 0b111 == 0
                && (((self.instr >> 22) & 1 == 0
                    && (self.instr >> 7) & 0b11111 == 1
                    && (self.instr >> 4) & 1 == 1)
                    || ((self.instr >> 22) & 1 == 1
                        && (self.instr >> 7) & 1 == 1
                        && (self.instr >> 4) & 1 == 1))
            {
                #[cfg(feature = "debug_instr")]
                self.debug("        halfword_signed_transfer");
                self.execute_halfword_signed_transfer(bus)
            } else if (self.instr >> 23) & 0b11111 == 0b00010
                && (self.instr >> 16) & 0b111111 == 0b001111
                && self.instr & 0b111111111111 == 0
            {
                // msr and mrs
                #[cfg(feature = "debug_instr")]
                self.debug("        MRS");
                self.execute_mrs_psr2reg()
            } else if ((self.instr >> 23) & 0b11111 == 0b00110 && (self.instr >> 20) & 0b11 == 0b10)
                || ((self.instr >> 23) & 0b11111 == 0b00010
                    && (self.instr >> 20) & 0b11 == 0b10
                    && (self.instr >> 4) & 0b111111111111 == 0b111100000000)
            {
                #[cfg(feature = "debug_instr")]
                self.debug("        MSR");
                self.execute_msr()
            } else {
                match (self.instr >> 25) & 0b111 {
                    0b000 | 0b001 => {
                        #[cfg(feature = "debug_instr")]
                        self.debug("        dataproc");
                        self.execute_dataproc()
                    }
                    0b101 => {
                        #[cfg(feature = "debug_instr")]
                        self.debug("        branch");
                        self.execute_branch()
                    }
                    0b100 => {
                        #[cfg(feature = "debug_instr")]
                        self.debug("        block data transfer");
                        self.execute_block_data_transfer(bus)
                    }
                    _ => {
                        print!(
                            "Error undefined instruction {:#034b} at pc {}",
                            self.instr, self.actual_pc
                        );
                        0
                    }
                }
            };
        } else {
            cur_cycles = 1;
            #[cfg(feature = "debug_instr")]
            self.debug("cond check failed, no instruction execution");
        }

        /*else if (self.instr >> 23) & 0b11111 == 0b00010 && (self.instr >> 12) & 0b1111111111 == 0b1010011111 && (self.instr >> 4) & 0b1111111111 == 0{
            self.debug("        MSR reg2psr");
            self.execute_msr_reg2psr()
        } */
        //else if (self.instr >> 26) & 0b11 == 0 && (self.instr >> 23) & 0b11 == 0b10 && (self.instr >> 12) & 0b1111111111 == 0b1010001111{

        if self.increment_pc {
            self.actual_pc += 0b100;
            #[cfg(feature = "debug_instr")]
            self.debug(" increment pc\n");
        };
        #[cfg(feature = "debug_instr")]
        self.debug("\n\n");

        cur_cycles
    }

    // ---------- branches
    #[inline(always)]
    fn execute_branch(&mut self) -> u32 {
        // link bit set
        if (self.instr >> 24) & 1 == 1 {
            self.set_reg(14, self.actual_pc + 4);
            //info!("   actual_pc: {:#x}, reg14: {:#x}", self.actual_pc, self.reg[Register::R14 as usize]);
        }
        let mut offset = (self.instr << 8) >> 6;
        if (offset >> 25) & 1 == 1 {
            offset |= 0b111111 << 26;
        }
        self.actual_pc = (Wrapping(self.read_pc()) + Wrapping(offset)).0;
        self.pipeline_instr.clear();
        self.increment_pc = false;
        3
    }

    #[inline(always)]
    fn execute_branch_exchange(&mut self) -> u32 {
        assert!(!self.read_flag(Flag::T));
        let addr = self.read_reg(self.instr & 0b1111);
        if addr & 1 > 0 {
            self.set_flag(Flag::T, true);
        };
        self.actual_pc = (addr >> 1) << 1;
        self.pipeline_instr.clear();
        self.increment_pc = false;
        3
    }

    // ---------- data processing

    // returns number of clock cycles
    #[inline(always)]
    fn execute_dataproc(&mut self) -> u32 {
        let mut cur_cycles =
            1 + self.process_reg_dest() + self.process_operand2() + self.process_operand1();
        //print!(" reg_dest: {}, operand1: {:x}, operand2: {:x}", self.reg_dest, self.operand1, self.operand2);

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
            _ => 0,
        };

        cur_cycles
    }

    //TODO: note copy to CPSR when dest is R15
    #[inline(always)]
    fn op_adc(&mut self) -> u32 {
        let res = Wrapping(self.operand1)
            + Wrapping(self.operand2)
            + Wrapping(self.read_flag(Flag::C) as u32);
        let res = res.0;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            //self.set_flag(Flag::C, (self.operand1 >> 31 > 0 || self.operand2 >> 31 > 0) && res >> 31 == 0);
            self.set_flag(Flag::C, self.operand1 > res || self.operand2 > res);
            self.set_flag(
                Flag::V,
                (self.operand1 >> 31 == self.operand2 >> 31) && res >> 31 != self.operand1 >> 31,
            );
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_add(&mut self) -> u32 {
        //if self.reg_dest == 0 {
        //    info!("add PC: {:#010x}\n  instr: {:#034b}\n   operand2: {:#x}", self.actual_pc, self.instr, self.operand2);
        //}
        let res = Wrapping(self.operand1) + Wrapping(self.operand2);
        let res = res.0;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            //self.set_flag(Flag::C, (self.operand1 >> 31 > 0 || self.operand2 >> 31 > 0) && res >> 31 == 0);
            self.set_flag(Flag::C, self.operand1 > res || self.operand2 > res);
            self.set_flag(
                Flag::V,
                (self.operand1 >> 31 == self.operand2 >> 31) && res >> 31 != self.operand1 >> 31,
            );
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_and(&mut self) -> u32 {
        let res = self.operand1 & self.operand2;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_bic(&mut self) -> u32 {
        let res = self.operand1 & !self.operand2;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_cmn(&mut self) -> u32 {
        let res = Wrapping(self.operand1) + Wrapping(self.operand2);
        let res = res.0;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            //self.set_flag(Flag::C, (self.operand1 >> 31 > 0 || self.operand2 >> 31 > 0) && res >> 31 == 0);
            self.set_flag(Flag::C, self.operand1 > res || self.operand2 > res);
            self.set_flag(
                Flag::V,
                (self.operand1 >> 31 == self.operand2 >> 31) && res >> 31 != self.operand1 >> 31,
            );
        }
        if self.reg_dest == 0b1111 {
            if let Some(reg) = self.spsr_map.get(&self.op_mode) {
                let spsr = self.reg[*reg as usize];
                self.set_cpsr(spsr);
            }
        }
        //self._op_set_pc(res);
        0
    }

    #[inline(always)]
    fn op_cmp(&mut self) -> u32 {
        let res = Wrapping(self.operand1) - Wrapping(self.operand2);
        //print!(" op1: {}, op2: {}, res: {}, set_cond: {}", self.operand1, self.operand2, res, self.dataproc_set_cond());
        let res = res.0;
        //info!("{:#x} {:#x} {:#x}", self.operand1, self.operand2, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.operand2 <= self.operand1);
            self.set_flag(
                Flag::V,
                (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand2 >> 31,
            );
        }
        if self.reg_dest == 0b1111 {
            if let Some(reg) = self.spsr_map.get(&self.op_mode) {
                let spsr = self.reg[*reg as usize];
                self.set_cpsr(spsr);
            }
        }
        //self._op_set_pc(res);
        //info!("{}", self.read_flag(Flag::Z));
        0
    }

    #[inline(always)]
    fn op_eor(&mut self) -> u32 {
        let res = self.operand1 ^ self.operand2;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_mov(&mut self) -> u32 {
        //if self.reg_dest == 0 {
        //    info!("PC: {:#010x}\n  instr: {:#034b}\n   operand2: {:#x}", self.actual_pc, self.instr, self.operand2);
        //}
        //if self.reg_dest == 8 && self.operand2 == 16 {
        //    info!("PC: {:#010x}\n  instr: {:#034b}", self.actual_pc, self.instr);
        //}
        self.set_reg(self.reg_dest, self.operand2);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, self.operand2 >> 31 > 0);
            self.set_flag(Flag::Z, self.operand2 == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        self._op_set_pc(self.operand2);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_mvn(&mut self) -> u32 {
        let res = !self.operand2;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_orr(&mut self) -> u32 {
        let res = self.operand1 | self.operand2;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_rsb(&mut self) -> u32 {
        let res = Wrapping(self.operand2) - Wrapping(self.operand1);
        let res = res.0;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.operand1 <= self.operand2);
            self.set_flag(
                Flag::V,
                (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand1 >> 31,
            );
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_rsc(&mut self) -> u32 {
        let flag_c = self.read_flag(Flag::C);
        let res = Wrapping(self.operand2) - Wrapping(self.operand1) + Wrapping(flag_c as u32)
            - Wrapping(1);
        let res = res.0;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            //self.set_flag(Flag::C, if self.operand1 > self.operand2 {false} else {true});

            let overflow =
                (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand1 >> 31;
            if flag_c {
                self.set_flag(Flag::C, self.operand1 <= self.operand2);
                //self.set_flag(Flag::V, overflow);
            } else {
                self.set_flag(Flag::C, self.operand1 < self.operand2);
                //self.set_flag(Flag::V, (!overflow && res == 0) || (overflow && res > 0));
            }
            self.set_flag(Flag::V, overflow);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_sbc(&mut self) -> u32 {
        let flag_c = self.read_flag(Flag::C);
        let res = Wrapping(self.operand1) - Wrapping(self.operand2) + Wrapping(flag_c as u32)
            - Wrapping(1);
        let res = res.0;
        //info!("pc:{:#x} op1: {:#x} op2: {:#x} flag_c: {}, res: {:#x}", self.actual_pc, self.operand1, self.operand2, flag_c as u32, res);

        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            //self.set_flag(Flag::C, if self.operand1 > self.operand2 {false} else {true});

            let overflow =
                (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand2 >> 31;
            if flag_c {
                self.set_flag(Flag::C, self.operand2 <= self.operand1);
                //self.set_flag(Flag::V, overflow);
            } else {
                self.set_flag(Flag::C, self.operand2 < self.operand1);
                //self.set_flag(Flag::V, (!overflow && res == 0) || (overflow && res > 0));
            }
            self.set_flag(Flag::V, overflow);
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_sub(&mut self) -> u32 {
        let res = Wrapping(self.operand1) - Wrapping(self.operand2);
        let res = res.0;
        self.set_reg(self.reg_dest, res);
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.operand2 <= self.operand1);
            self.set_flag(
                Flag::V,
                (self.operand1 >> 31 != self.operand2 >> 31) && res >> 31 == self.operand2 >> 31,
            );
        }
        self._op_set_pc(res);
        2 * (self.reg_dest == Register::R15 as u32) as u32
    }

    #[inline(always)]
    fn op_teq(&mut self) -> u32 {
        let res = self.operand1 ^ self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        if self.reg_dest == 0b1111 {
            if let Some(reg) = self.spsr_map.get(&self.op_mode) {
                let spsr = self.reg[*reg as usize];
                self.set_cpsr(spsr);
            }
        }
        0
    }

    #[inline(always)]
    fn op_tst(&mut self) -> u32 {
        let res = self.operand1 & self.operand2;
        if self.dataproc_set_cond() && self.reg_dest != Register::R15 as u32 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
            self.set_flag(Flag::C, self.shifter_carry > 0);
        }
        if self.reg_dest == 0b1111 {
            if let Some(reg) = self.spsr_map.get(&self.op_mode) {
                let spsr = self.reg[*reg as usize];
                self.set_cpsr(spsr);
            }
        }
        0
    }

    #[inline(always)]
    fn _op_set_pc(&mut self, res: u32) {
        if self.reg_dest == Register::R15 as u32 {
            self.actual_pc = res;
            self.pipeline_instr.clear();
            self.increment_pc = false;
            if self.dataproc_set_cond() {
                if let Some(reg) = self.spsr_map.get(&self.op_mode) {
                    let spsr = self.reg[*reg as usize];
                    self.set_cpsr(spsr);
                } else {
                    warn!("s bit should not be set");
                }
            }
        }
    }

    // ---------- MRS and MSR
    #[inline(always)]
    fn execute_mrs_psr2reg(&mut self) -> u32 {
        let reg = if (self.instr >> 22 & 1) == 0 {
            Register::Cpsr
        } else {
            match self.spsr_map.get(&self.op_mode) {
                Some(&opmode) => opmode,
                None => Register::Cpsr,
            }
        };
        let res = self.reg[reg as usize];
        self.reg_dest = (self.instr >> 12) & 0b1111;
        self.set_reg(self.reg_dest, res);
        1
    }

    // NOTE: inconsistencies between ARM7TDMI_data_sheet.pdf and cpu_technical_spec_long.pdf regarding MSR.
    // ARM7TDMI_data_sheet.pdf was chosen as the source of truth. TODO: check if this is the correct choice.
    /*fn execute_msr_reg2psr(&mut self) -> u32 {
        let reg_dest = if (self.instr >> 22 & 1) == 0 {Register::CPSR} else {*self.spsr_map.get(&self.op_mode).unwrap()};
        let res = self.read_reg(self.instr & 0b1111);
        self.reg[reg_dest as usize] = res;
        1
    }*/

    #[inline(always)]
    fn execute_msr(&mut self) -> u32 {
        let R = (self.instr >> 22 & 1) > 0;
        let reg_dest = if !R {
            Register::Cpsr
        } else {
            //info!("{} {:#034b}", self.op_mode as u32, self.reg[Register::CPSR as usize]);
            match self.spsr_map.get(&self.op_mode) {
                Some(&opmode) => opmode,
                None => {
                    warn!(
                        "msr called on R=1, but this mode has no SPSR {}",
                        self.instr
                    );
                    //Register::CPSR
                    return 1;
                }
            }
        };
        let res = if (self.instr >> 25) & 1 == 0 {
            // register
            self.read_reg(self.instr & 0b1111)
        } else {
            // immediate
            self.process_immediate_rotate();
            self.operand2
        };

        let mask = (self.instr >> 16) & 0b1111;
        if mask != 0b1001 && mask != 0b1000 {
            warn!("MSR with invalid mask");
            //return 1;
        }
        //info!("  pc: {:#x}, instr: {:#034b}, mask: {:#06b}", self.actual_pc, self.instr, mask);
        let mut cur = self.reg[reg_dest as usize];
        for i in 0..4 {
            let range = 0b11111111 << (i * 8);
            if (1 << i) & mask > 0 {
                cur &= !range;
                cur |= res & range;
            }
        }
        if !R {
            self.set_cpsr(cur);
        } else {
            self.reg[reg_dest as usize] = cur;
        }
        1
    }

    // ---------- multiplications
    #[inline(always)]
    fn execute_multiply(&mut self) -> u32 {
        self.reg_dest = (self.instr >> 16) & 0b1111;
        self.operand1 = self.read_reg((self.instr >> 12) & 0b1111);
        self.operand2 = self.read_reg((self.instr >> 8) & 0b1111);
        let operand3 = self.read_reg((self.instr) & 0b1111);

        let mut cur_cycles;

        let res = if (self.instr >> 21) & 1 > 0 {
            cur_cycles = 2;
            (Wrapping(operand3) * Wrapping(self.operand2) + Wrapping(self.operand1)).0
        } else {
            cur_cycles = 1;
            (Wrapping(operand3) * Wrapping(self.operand2)).0
        };

        self.set_reg(self.reg_dest, res);

        if (self.instr >> 20) & 1 == 1 {
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
        }

        cur_cycles += if self.operand2 >> 8 == 0 || self.operand2 >> 8 == (1 << 24) - 1 {
            1
        } else if self.operand2 >> 16 == 0 || self.operand2 >> 16 == (1 << 16) - 1 {
            2
        } else if self.operand2 >> 24 == 0 || self.operand2 >> 24 == (1 << 8) - 1 {
            3
        } else {
            4
        };

        cur_cycles
    }

    #[inline(always)]
    fn execute_multiply_long(&mut self) -> u32 {
        let reg_dest_hi = (self.instr >> 16) & 0b1111;
        let reg_dest_lo = (self.instr >> 12) & 0b1111;
        let operand2 = self.read_reg((self.instr >> 8) & 0b1111);
        let operand3 = self.read_reg((self.instr) & 0b1111);
        let operand1 =
            ((self.read_reg(reg_dest_hi) as u64) << 32) + self.read_reg(reg_dest_lo) as u64;

        let mut cur_cycles;
        let unsigned = (self.instr >> 22) & 1 == 0;

        let res = if (self.instr >> 21) & 1 > 0 {
            cur_cycles = 2;
            if unsigned {
                operand1 + operand3 as u64 * operand2 as u64
            } else {
                (operand1 as i64 + operand3 as i32 as i64 * operand2 as i32 as i64) as u64
            }
        } else {
            cur_cycles = 1;
            if unsigned {
                operand3 as u64 * operand2 as u64
            } else {
                (operand3 as i32 as i64 * operand2 as i32 as i64) as u64
            }
        };

        self.set_reg(reg_dest_hi, (res >> 32) as u32);
        self.set_reg(reg_dest_lo, res as u32);

        if (self.instr >> 20) & 1 == 1 {
            self.set_flag(Flag::N, res >> 63 > 0);
            self.set_flag(Flag::Z, res == 0);
        }

        cur_cycles += if operand2 >> 8 == 0 || (!unsigned && operand2 >> 8 == (1 << 24) - 1) {
            1
        } else if operand2 >> 16 == 0 || (!unsigned && operand2 >> 16 == (1 << 16) - 1) {
            2
        } else if operand2 >> 24 == 0 || (!unsigned && operand2 >> 24 == (1 << 8) - 1) {
            3
        } else {
            4
        };

        cur_cycles
    }

    // ---------- data transfers
    #[inline(always)]
    fn execute_ldr_str(&mut self, bus: &mut Bus) -> u32 {
        //info!("{:#034b}", self.instr);
        //self.instr &= !(1 << 21);
        let mut cycles = 0;
        // I flag
        let offset = if (self.instr >> 25) & 1 > 0 {
            // NOTE: double check if cycles are added here
            //cycles +=
            self.process_reg_rotate(false);
            //self.debug(&format!(" reg rotate operand2: {:#x}", self.operand2));
            self.operand2
        } else {
            self.instr & 0b111111111111
        };
        // base reg
        let base_reg = (self.instr >> 16) & 0b1111;
        let mut addr = self.read_reg(base_reg);

        // U flag
        let offset_addr = if (self.instr >> 23) & 1 == 0 {
            Wrapping(addr) - Wrapping(offset)
        } else {
            Wrapping(addr) + Wrapping(offset)
        };
        let offset_addr = offset_addr.0;

        // P flag
        let P = (self.instr >> 24) & 1 == 1;
        if P {
            addr = offset_addr;
        }

        let L = (self.instr >> 20) & 1 == 1;
        let B = (self.instr >> 22) & 1 == 1;

        let rotate = (addr & 0b11) * 8;

        let addr = if !B {
            (addr as usize) & !0b11
        } else {
            addr as usize
        };

        let reg = (self.instr >> 12) & 0b1111;

        let store_res = self.read_reg(reg) + if reg == Register::R15 as u32 { 4 } else { 0 };

        //self.debug(&format!(" addr: {:#x}, L: {}, store_res: {:#x}, rd: {}, IE: {:#018b}", addr, L, store_res, reg, bus.read_halfword(0x4000200)));

        /*#[cfg(feature="debug_instr")]
        if !L && addr < 0x004000{
            self.debug_cnt += 2;
            self.print_pc(bus);
            self.debug(&format!(" reg: {}, base_reg: {}, L: {}, B: {}, W: {}, P: {}, addr: {:x}, offset: {:x}, offset_addr: {:x}", reg, base_reg, L, B, (self.instr >> 21) & 1 == 1, (self.instr >> 24) & 1 == 1, addr, offset, offset_addr));
        }*/

        // W flag
        if !P || (self.instr >> 21) & 1 == 1 {
            //if (self.instr >> 21) & 1 == 1 {
            self.set_reg(base_reg, offset_addr);
        };

        match (L, B) {
            // register -> memory, byte
            (false, true) => {
                bus.store_byte(addr, store_res as u8);
                cycles += 2;
            }
            // register -> memory, word
            (false, false) => {
                //let addr = (addr >> 2) << 2;

                bus.store_word(addr, store_res);
                cycles += 2;
            }
            // memory -> register, byte
            (true, true) => {
                let res = bus.read_byte(addr);
                self.set_reg(reg, res as u32);
                cycles += 3;
            }
            // memory -> register, word
            (true, false) => {
                let mut res = bus.read_word(addr).rotate_right(rotate);
                if reg == Register::R15 as u32 {
                    res &= 0xfffffffc;
                    self.actual_pc = res;
                    // NOTE: may not be correct, maybe comment out
                    self.pipeline_instr.clear();
                    self.increment_pc = false;
                    cycles += 2;
                }
                self.set_reg(reg, res);
                /*
                if (addr & 0b10) > 0 {
                    let hi = bus.read_halfword(addr) as u32;
                    let lo = bus.read_halfword(addr + 2) as u32;
                    self.set_reg(reg, lo + (hi << 16));
                }
                else{
                    let res = bus.read_word(addr);
                    self.set_reg(reg, res);
                }
                */
                cycles += 3;
            }
        };

        //if L && reg == Register::R15 as u32 {
        //    cycles += 2;
        //}

        cycles
    }

    #[inline(always)]
    fn execute_halfword_signed_transfer(&mut self, bus: &mut Bus) -> u32 {
        let offset = if (self.instr >> 22) & 1 == 0 {
            self.read_reg(self.instr & 0b1111)
        } else {
            let hi = (self.instr >> 8) & 0b1111;
            let lo = self.instr & 0b1111;
            lo + (hi << 4)
        };
        // base reg
        let base_reg = (self.instr >> 16) & 0b1111;
        let mut addr = self.read_reg(base_reg);
        //self.debug(&format!(" org_addr: {:#x},", addr));
        // U flag
        let offset_addr = if (self.instr >> 23) & 1 == 0 {
            addr - offset
        } else {
            addr + offset
        };

        // P flag
        let P = (self.instr >> 24) & 1 == 1;
        if P {
            addr = offset_addr;
        }

        let L = (self.instr >> 20) & 1 == 1;
        let S = (self.instr >> 6) & 1 == 1;
        let H = (self.instr >> 5) & 1 == 1;

        let rotate = 8 * (addr & 1);
        let addr = if H { addr as usize & !1 } else { addr as usize };

        let reg = (self.instr >> 12) & 0b1111;

        let store_res = self.read_reg(reg);

        //self.debug(&format!(" addr: {:#x}, L: {}, H: {}, store_res: {:#x}, rd: {}", addr, L, H, store_res, reg));

        if !P || (self.instr >> 21) & 1 == 1 {
            //if (self.instr >> 21) & 1 == 1 {
            self.set_reg(base_reg, offset_addr);
        };

        match (L, S, H) {
            // register -> memory, byte (STRH)
            (false, false, true) => {
                bus.store_halfword(addr, store_res as u16);
            }
            // LDRH
            (true, false, true) => {
                //self.set_reg(reg, bus.read_halfword(addr) as u32);
                self.set_reg(reg, (bus.read_halfword(addr) as u32).rotate_right(rotate));
            }
            // LDRSH
            (true, true, true) => {
                //let mut res = bus.read_halfword(addr) as u32;
                let mut res = (bus.read_halfword(addr) as u32).rotate_right(rotate);
                //info!("org: {:#034b} res: {:#034b}", bus.read_halfword(addr), res);
                if rotate == 0 && (res >> 15) & 1 > 0 {
                    res |= ((1 << 16) - 1) << 16;
                }
                // only 2 values of rotate: 0 and 8
                else if rotate == 8 && (res >> 7) & 1 > 0 {
                    res |= !0b11111111;
                }
                //info!("res: {:#b}", res);
                self.set_reg(reg, res);
            }
            // LDRSB
            (true, true, false) => {
                let mut res = bus.read_byte(addr) as u32;
                if (res >> 7) & 1 > 0 {
                    res |= ((1 << 24) - 1) << 8;
                }
                self.set_reg(reg, res);
            }
            _ => {
                warn!("Error undefined combination in execute_halfword_signed_transfer with instr {:#034b} at pc {}\n", self.instr, self.actual_pc);
            }
        };

        // W flag
        //self.debug(&format!(" offset_addr: {:#x},", offset_addr));

        if (L, S, H) == (false, false, true) {
            2
        } else if reg == Register::R15 as u32 {
            5
        } else {
            3
        }
    }

    #[inline(always)]
    fn execute_block_data_transfer(&mut self, bus: &mut Bus) -> u32 {
        // base reg
        let base_reg = (self.instr >> 16) & 0b1111;
        let mut addr = self.read_reg(base_reg);

        let L = (self.instr >> 20) & 1 == 1;
        let W = (self.instr >> 21) & 1 == 1;
        let S = (self.instr >> 22) & 1 == 1;
        let U = (self.instr >> 23) & 1 == 1;
        let pre = (self.instr >> 24) & 1 == 1;

        //self.debug(&format!(" addr: {:#x}, L: {}, W: {}, U: {}", addr, L, W, U));
        //info!("{}",&format!(" addr: {:#x}, L: {}, W: {}, U: {}, pre: {}", addr, L, W, U, pre));

        let reg_list = self.instr & 0b1111111111111111;

        // undefined operation: no registers in list
        //let mut zero_reg_list = false;
        //if reg_list == 0{
        //    reg_list = 1 << 15;
        //    zero_reg_list = true;
        //}

        let mut cnt = 0;
        let r15_appear = (1 << 15) & reg_list > 0;

        for i in 0..16 {
            if (1 << i) & reg_list > 0 {
                cnt += 1;
            }
        }

        // undefined operation: no registers in list
        //if zero_reg_list{
        //    cnt = 16;
        //}

        let offset_addr = if U { addr + 4 * cnt } else { addr - 4 * cnt };
        if !U {
            addr = offset_addr;
        }

        let mut addr = (addr as usize >> 2) << 2;

        let delt = match (pre, U) {
            (true, true) => 4,
            (false, true) => 0,
            (true, false) => 0,
            (false, false) => 4,
        };

        cnt = 0;

        //if W {
        //    self.set_reg(base_reg, offset_addr);
        //}

        for i in 0..16 {
            if (1 << i) & reg_list > 0 {
                let reg = self.reg_map[if S && (!r15_appear || !L) {
                    OperatingMode::Usr as usize
                } else {
                    self.op_mode as usize
                }][i as usize];
                if L {
                    self.reg[reg as usize] = bus.read_word(addr + delt);
                    if i == 15 {
                        self.reg[reg as usize] &= 0xfffffffc;
                        // NOTE: may not be correct, maybe comment out
                        self.actual_pc = self.reg[reg as usize];
                        self.pipeline_instr.clear();
                        self.increment_pc = false;
                    }
                } else {
                    let mut res = self.reg[reg as usize];
                    // account for pc being 12 bytes higher than current position
                    if i == 15 {
                        res += 4;
                    }
                    bus.store_word(addr + delt, res);
                }
                if W && cnt == 0 {
                    self.set_reg(base_reg, offset_addr);
                }
                addr += 4;
                cnt += 1;
            }
        }

        if S && r15_appear && L {
            self.set_cpsr(self.reg[self.spsr_map[&self.op_mode] as usize]);
        }

        if L {
            if r15_appear {
                4 + cnt
            } else {
                2 + cnt
            }
        } else {
            1 + cnt
        }
    }

    #[inline(always)]
    fn execute_swp(&mut self, bus: &mut Bus) -> u32 {
        let B = (self.instr >> 22) & 1 == 1;
        self.reg_dest = (self.instr >> 12) & 0b1111;
        let res = self.read_reg(self.instr & 0b1111);
        let addr = self.read_reg((self.instr >> 16) & 0b1111) as usize;

        if B {
            self.set_reg(self.reg_dest, bus.read_byte(addr) as u32);
            bus.store_byte(addr, res as u8);
        } else {
            let rotate = (addr as u32 & 0b11) << 3;
            let addr = addr & !(0b11);
            self.set_reg(self.reg_dest, bus.read_word(addr).rotate_right(rotate));
            bus.store_word(addr, res);
        }

        4
    }

    // ---------- miscellaneous helpers
    #[inline(always)]
    fn check_cond(&self, cond: u32) -> bool {
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
            0b1010 => self.read_flag(Flag::N) == self.read_flag(Flag::V),
            0b1011 => self.read_flag(Flag::N) != self.read_flag(Flag::V),
            0b1100 => {
                !self.read_flag(Flag::Z) && (self.read_flag(Flag::N) == self.read_flag(Flag::V))
            }
            0b1101 => {
                self.read_flag(Flag::Z) || (self.read_flag(Flag::N) != self.read_flag(Flag::V))
            }
            0b1110 => true,
            _ => {
                warn!(
                    "cond field not valid: instr: {:#034b}, pc: {:#x}",
                    self.instr, self.actual_pc
                );
                false
            }
        }
    }

    #[inline(always)]
    fn dataproc_set_cond(&self) -> bool {
        // check for thumb mode, so we can re-use the op_ methods for thumb
        (self.read_flag(Flag::T) && self.thumb_modify_flags) || (self.instr >> 20) & 1 > 0
    }

    // modifies self.operand2. returns number of extra cycles (0)
    #[inline(always)]
    fn process_immediate_rotate(&mut self) -> u32 {
        let cur = self.instr & 0b11111111;
        let rotate = ((self.instr >> 8) & 0b1111) * 2;
        self.operand2 = cur.rotate_right(rotate);
        if rotate > 0 {
            self.shifter_carry = (self.operand2 >> 31) & 1;
        }
        0
    }

    #[inline(always)]
    fn process_reg_rotate(&mut self, is_dataproc: bool) -> u32 {
        // register is used
        //let reg = &self.reg_map.get(&self.op_mode).unwrap()[self.instr as usize & 0b1111];
        //let cur = self.reg[*reg as usize];

        //
        let is_immediate = (self.instr >> 4) & 1 == 0;

        let mut shift_amount = if is_immediate {
            // the shift amount is a literal; ie not a register
            (self.instr >> 7) & 0b11111
        } else {
            // the shift amount is stored in the lowest byte in a register
            if is_dataproc {
                self.set_reg(15, self.actual_pc + 12);
            }
            //let reg = (self.instr >> 8) & 0b1111;
            //let reg = &self.reg_map.get(&self.op_mode).unwrap()[reg as usize];
            //shift_amount = self.reg[*reg as usize] & 0b11111111;
            self.read_reg((self.instr >> 8) & 0b1111) & 0b11111111
        };

        let cur = self.read_reg(self.instr & 0b1111);

        let shift_type = (self.instr >> 5) & 0b11;
        match shift_type {
            // logical left
            0b00 => {
                if shift_amount > 32 {
                    self.operand2 = 0;
                    self.shifter_carry = 0;
                } else {
                    self.operand2 = if shift_amount < 32 {
                        cur << shift_amount
                    } else {
                        0
                    };
                    if shift_amount > 0 {
                        self.shifter_carry = (cur >> (32 - shift_amount)) & 1;
                    }
                }
            }
            0b01 => {
                if shift_amount == 0 {
                    if is_immediate {
                        self.operand2 = 0;
                        self.shifter_carry = cur >> 31;
                    } else {
                        self.operand2 = cur;
                    }
                } else if shift_amount > 32 {
                    self.operand2 = 0;
                    self.shifter_carry = 0;
                } else {
                    self.operand2 = if shift_amount < 32 {
                        cur >> shift_amount
                    } else {
                        0
                    };
                    self.shifter_carry = (cur >> (shift_amount - 1)) & 1;
                }
            }
            0b10 => {
                if shift_amount == 0 && !is_immediate {
                    self.operand2 = cur;
                } else {
                    if shift_amount == 0 || shift_amount > 32 {
                        shift_amount = 32;
                    }
                    //shift_amount = min(shift_amount, 32);

                    self.operand2 = if shift_amount == 32 {
                        0
                    } else {
                        cur >> shift_amount
                    };
                    self.shifter_carry = (cur >> (shift_amount - 1)) & 1;

                    if cur >> 31 & 1 > 0 {
                        self.operand2 |= (0xffffffff >> (32 - shift_amount)) << (32 - shift_amount);
                    }
                }
            }
            0b11 => {
                if shift_amount == 0 && !is_immediate {
                    self.operand2 = cur;
                } else if shift_amount > 0 {
                    let shift_mod = shift_amount & 0b11111;
                    self.shifter_carry =
                        (cur >> (if shift_mod > 0 { shift_mod } else { 32 } - 1)) & 1;
                    self.operand2 = cur.rotate_right(shift_amount);
                } else {
                    self.shifter_carry = cur & 1;
                    self.operand2 = (cur >> 1) | ((self.read_flag(Flag::C) as u32) << 31)
                }
            }
            _ => {}
        }

        !is_immediate as u32
    }

    // returns extra cycle count. Stores the result into self.operand2. Stores shifter carry into self.shifter_carry.
    #[inline(always)]
    fn process_operand2(&mut self) -> u32 {
        self.shifter_carry = self.read_flag(Flag::C) as u32;
        let is_immediate = (self.instr >> 25) & 1 > 0;
        // immediate value is used
        if is_immediate {
            self.process_immediate_rotate()
        } else {
            self.process_reg_rotate(true)
        }
    }

    #[inline(always)]
    fn process_operand1(&mut self) -> u32 {
        let reg = (self.instr >> 16) & 0b1111;
        self.operand1 = self.read_reg(reg);
        0
    }

    #[inline(always)]
    fn process_reg_dest(&mut self) -> u32 {
        self.reg_dest = (self.instr >> 12) & 0b1111;
        0
    }

    // ------------- THUMB INSTRUCTIONS -----------

    /*fn generate_arm_decode_table() -> Vec<fn(&mut Cpu, &mut Bus) -> u32> {
        let mut res = Vec::<fn(&mut Cpu, &mut Bus) -> u32>::with_capacity(256);
        for i in 0..256u32 {
            let instr = i << 8;
            let f = if (instr >> 11) & 0b11111 == 0b00011 {
                Cpu::execute_thumb_add_sub_imm3
            } else if (instr >> 8) == 0b11011111 {
                Cpu::execute_thumb_software_interrupt
            } else if (instr >> 10) & 0b111111 == 0b010000 {
                Cpu::execute_thumb_alu_general
            } else if (instr >> 10) & 0b111111 == 0b010001 {
                Cpu::execute_thumb_hi_bx
            } else if (instr >> 11) & 0b11111 == 0b01001 {
                Cpu::execute_thumb_pc_relative_load
            } else if (instr >> 12) & 0b1111 == 0b0101 && (instr >> 9) & 1 == 0 {
                Cpu::execute_thumb_load_store_reg_offset
            } else if (instr >> 12) & 0b1111 == 0b0101 && (instr >> 9) & 1 == 1 {
                Cpu::execute_thumb_load_store_signed
            } else if (instr >> 8) & 0b11111111 == 0b10110000 {
                Cpu::execute_thumb_sp_offset
            } else if (instr >> 9) & 0b11 == 0b10 && (instr >> 12) & 0b1111 == 0b1011 {
                Cpu::execute_thumb_push_pop
            } else if (instr >> 11) & 0b11111 == 0b11100 {
                Cpu::execute_thumb_uncond_branch
            } else {
                match (instr >> 12) & 0b1111 {
                    0b0001 | 0b0000 => Cpu::execute_thumb_lsl_lsr_asr_imm5,
                    0b0010 | 0b0011 => Cpu::execute_thumb_mov_cmp_add_sub_imm8,
                    0b0111 | 0b0110 => Cpu::execute_thumb_load_store_imm5,
                    0b1000 => Cpu::execute_thumb_load_store_halfword_imm5,
                    0b1001 => Cpu::execute_thumb_load_store_sp,
                    0b1010 => Cpu::execute_thumb_load_address,
                    0b1100 => Cpu::execute_thumb_load_store_multiple,
                    0b1101 => Cpu::execute_thumb_cond_branch,
                    0b1111 => Cpu::execute_thumb_uncond_branch_link,
                    _ => Cpu::execute_thumb_undefined_instr,
                }
            };
            res.push(f);
        }
        res
    }*/

    #[inline(always)]
    fn fetch_thumb_instr(&mut self, bus: &mut Bus){
        if self.pipeline_instr.is_empty(){
            let data = bus.read_halfword(self.actual_pc as usize) as u32;
            self.pipeline_instr.push_back(data + (data << 16));
            let data = bus.read_halfword(self.actual_pc as usize + 2) as u32;
            self.pipeline_instr.push_back(data + (data << 16));
        }
        let data = bus.read_halfword(self.actual_pc as usize + 4) as u32;
        self.pipeline_instr.push_back(data + (data << 16));
        self.instr = self.pipeline_instr.pop_front().unwrap() as u16 as u32;
        if self.actual_pc < 0x4000 {
            self.last_fetched_bios_instr =
                bus.read_word_raw(self.actual_pc as usize + 4, MemoryRegion::Bios) as u32;
        }
    }

    #[inline(always)]
    fn decode_execute_instruction_thumb(&mut self, bus: &mut Bus) -> u32 {
        // get rid of the trailing bits, these may be set to 1 but must always be treated as 0
        self.actual_pc &= !0b01;
        self.fetch_thumb_instr(bus);
        self.set_pc(self.actual_pc + 4);

        let mut cur_cycles = 0;

        self.increment_pc = true;
        self.thumb_modify_flags = true;

        #[cfg(feature = "debug_instr")]
        self.print_pc(bus);

        // for compatibility with thumb op instructions
        self.shifter_carry = 0;

        //cur_cycles += self.arm_instr_table[self.instr as usize >> 8](self, bus);
        cur_cycles += if (self.instr >> 11) & 0b11111 == 0b00011 {
            self.execute_thumb_add_sub_imm3(bus)
        } else if (self.instr >> 8) == 0b11011111 {
            self.execute_thumb_software_interrupt(bus)
        } else if (self.instr >> 10) & 0b111111 == 0b010000 {
            self.execute_thumb_alu_general(bus)
        } else if (self.instr >> 10) & 0b111111 == 0b010001 {
            self.execute_thumb_hi_bx(bus)
        } else if (self.instr >> 11) & 0b11111 == 0b01001 {
            self.execute_thumb_pc_relative_load(bus)
        } else if (self.instr >> 12) & 0b1111 == 0b0101 && (self.instr >> 9) & 1 == 0 {
            self.execute_thumb_load_store_reg_offset(bus)
        } else if (self.instr >> 12) & 0b1111 == 0b0101 && (self.instr >> 9) & 1 == 1 {
            self.execute_thumb_load_store_signed(bus)
        } else if (self.instr >> 8) & 0b11111111 == 0b10110000 {
            self.execute_thumb_sp_offset(bus)
        } else if (self.instr >> 9) & 0b11 == 0b10 && (self.instr >> 12) & 0b1111 == 0b1011 {
            self.execute_thumb_push_pop(bus)
        } else if (self.instr >> 11) & 0b11111 == 0b11100 {
            self.execute_thumb_uncond_branch(bus)
        } else {
            match (self.instr >> 12) & 0b1111 {
                0b0001 | 0b0000 => self.execute_thumb_lsl_lsr_asr_imm5(bus),
                0b0010 | 0b0011 => self.execute_thumb_mov_cmp_add_sub_imm8(bus),
                0b0111 | 0b0110 => self.execute_thumb_load_store_imm5(bus),
                0b1000 => self.execute_thumb_load_store_halfword_imm5(bus),
                0b1001 => self.execute_thumb_load_store_sp(bus),
                0b1010 => self.execute_thumb_load_address(bus),
                0b1100 => self.execute_thumb_load_store_multiple(bus),
                0b1101 => self.execute_thumb_cond_branch(bus),
                0b1111 => self.execute_thumb_uncond_branch_link(bus),
                _ => self.execute_thumb_undefined_instr(bus),
            }
        };
        /*if (self.instr >> 11) & 0b11111 == 0b00011 {
            self.debug("        thumb ADD SUB");
            self.execute_thumb_add_sub_imm3()
        }
        else if (self.instr >> 8) == 0b11011111 {
            self.debug("        thumb SWI");
            self.execute_software_interrupt()
        }
        else if (self.instr >> 10) & 0b111111 == 0b010000 {
            self.debug("        thumb ALU general");
            self.execute_thumb_alu_general()
        }
        else if (self.instr >> 10) & 0b111111 == 0b010001 {
            self.debug("        thumb Hi reg operations or BX");
            self.execute_thumb_hi_bx()
        }
        else if (self.instr >> 11) & 0b11111 == 0b01001 {
            self.debug("        thumb pc relative load");
            self.execute_thumb_pc_relative_load(bus)
        }
        else if (self.instr >> 12) & 0b1111 == 0b0101 && (self.instr >> 9) & 1 == 0{
            self.debug("        thumb load/store reg offset");
            self.execute_thumb_load_store_reg_offset(bus)
        }
        else if (self.instr >> 12) & 0b1111 == 0b0101 && (self.instr >> 9) & 1 == 1{
            self.debug("        thumb load/store reg signed byte/halfword");
            self.execute_thumb_load_store_signed(bus)
        }
        else if (self.instr >> 8) & 0b11111111 == 0b10110000{
            self.debug("        thumb sp offset");
            self.execute_thumb_sp_offset()
        }
        else if (self.instr >> 9) & 0b11 == 0b10 && (self.instr >> 12) & 0b1111 == 0b1011{
            self.debug("        thumb push/pop");
            self.execute_thumb_push_pop(bus)
        }
        else if (self.instr >> 11) & 0b11111 == 0b11100 {
            self.debug("        thumb uncond branch");
            self.execute_thumb_uncond_branch()
        }
        else{
            match (self.instr >> 12) & 0b1111 {
                0b0001 | 0b0000 => {
                    self.debug("        thumb LSL LSR ASR imm5");
                    self.execute_thumb_lsl_lsr_asr_imm5()
                },
                0b0010 | 0b0011 => {
                    self.debug("        thumb MOV CMP ADD SUB imm8");
                    self.execute_thumb_mov_cmp_add_sub_imm8()
                },
                0b0111 | 0b0110 => {
                    self.debug("        thumb load/store reg imm5");
                    self.execute_thumb_load_store_imm5(bus)
                },
                0b1000 => {
                    self.debug("        thumb load/store halfword imm5");
                    self.execute_thumb_load_store_halfword_imm5(bus)
                },
                0b1001 => {
                    self.debug("        thumb load/store word sp offset");
                    self.execute_thumb_load_store_sp(bus)
                },
                0b1010 => {
                    self.debug("        thumb load address sp/pc");
                    self.execute_thumb_load_address()
                },
                0b1100 => {
                    self.debug("        thumb multiple load/store");
                    self.execute_thumb_load_store_multiple(bus)
                },
                0b1101 => {
                    self.debug("        thumb cond branch");
                    self.execute_thumb_cond_branch()
                }
                0b1111 => {
                    self.debug("        thumb long branch and link");
                    self.execute_thumb_uncond_branch_link()
                }
                _ => {
                    print!("Error undefined instruction {:#034b} at pc {}", self.instr, self.actual_pc);
                    0
                }
            }
        };*/
        if self.increment_pc {
            self.actual_pc += 0b010;
        }

        #[cfg(feature = "debug_instr")]
        self.debug("\n\n");

        cur_cycles
    }

    #[inline(always)]
    fn execute_thumb_undefined_instr(&mut self, _: &mut Bus) -> u32 {
        print!(
            "Error undefined instruction {:#034b} at pc {}",
            self.instr, self.actual_pc
        );
        0
    }

    // ---------- move shifted register
    #[inline(always)]
    fn execute_thumb_lsl_lsr_asr_imm5(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb LSL LSR ASR imm5");
        self.reg_dest = self.instr & 0b111;
        self.operand1 = self.read_reg((self.instr >> 3) & 0b111);
        self.operand2 = (self.instr >> 6) & 0b11111;
        match (self.instr >> 11) & 0b11 {
            // LSL
            0b00 => self.op_thumb_lsl(),
            // LSR
            0b01 => self.op_thumb_lsr(),
            // ASR
            0b10 => self.op_thumb_asr(),
            _ => 0,
        };

        1
    }

    #[inline(always)]
    fn op_thumb_lsl(&mut self) -> u32 {
        let res = if self.operand2 == 0 {
            self.operand1
        } else if self.operand2 == 32 {
            self.set_flag(Flag::C, self.operand1 & 1 > 0);
            0
        } else if self.operand2 > 32 {
            self.set_flag(Flag::C, false);
            0
        } else {
            self.set_flag(Flag::C, (self.operand1 >> (32 - self.operand2)) & 1 > 0);
            self.operand1 << self.operand2
        };
        self.set_flag(Flag::N, res >> 31 > 0);
        self.set_flag(Flag::Z, res == 0);

        self.set_reg(self.reg_dest, res);

        0
    }

    #[inline(always)]
    fn op_thumb_lsr(&mut self) -> u32 {
        let res = if self.operand2 == 0 || self.operand2 == 32 {
            self.set_flag(Flag::C, (self.operand1 >> 31) & 1 > 0);
            0
        } else if self.operand2 > 32 {
            self.set_flag(Flag::C, false);
            0
        } else {
            self.set_flag(Flag::C, (self.operand1 >> (self.operand2 - 1)) & 1 > 0);
            self.operand1 >> self.operand2
        };
        //self.set_flag(Flag::C, (self.operand1 >> (self.operand2 - 1)) & 1 > 0);
        self.set_flag(Flag::N, res >> 31 > 0);
        self.set_flag(Flag::Z, res == 0);

        self.set_reg(self.reg_dest, res);

        0
    }

    #[inline(always)]
    fn op_thumb_asr(&mut self) -> u32 {
        let mut shift_amount = min(self.operand2, 32);
        if shift_amount == 0 {
            shift_amount = 32;
        }
        let mut res = if shift_amount == 32 {
            0
        } else {
            self.operand1 >> shift_amount
        };
        if self.operand1 >> 31 & 1 > 0 {
            res |= (0xffffffff >> (32 - shift_amount)) << (32 - shift_amount);
        }
        self.set_reg(self.reg_dest, res);

        //print!(" shift amount: {:#010x}", shift_amount);
        self.set_flag(Flag::C, (self.operand1 >> (shift_amount - 1)) & 1 > 0);
        self.set_flag(Flag::N, res >> 31 > 0);
        self.set_flag(Flag::Z, res == 0);

        0
    }

    #[inline(always)]
    fn op_thumb_ror(&mut self) -> u32 {
        let shift_amount = self.operand2 & 0b11111;
        let res = if self.operand2 == 0 {
            // do nothing
            self.read_reg(self.reg_dest)
        } else if shift_amount == 0 {
            self.set_flag(Flag::C, (self.operand1 >> 31) & 1 > 0);
            self.read_reg(self.reg_dest)
        } else {
            self.set_flag(Flag::C, (self.operand1 >> (shift_amount - 1)) & 1 > 0);
            self.operand1.rotate_right(shift_amount)
        };
        self.set_reg(self.reg_dest, res);
        self.set_flag(Flag::N, res >> 31 > 0);
        self.set_flag(Flag::Z, res == 0);

        0
    }

    // ---------- add, sub- imm3
    #[inline(always)]
    fn execute_thumb_add_sub_imm3(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb ADD SUB");
        self.reg_dest = self.instr & 0b111;
        let I = (self.instr >> 10) & 1 > 0;
        self.operand1 = self.read_reg((self.instr >> 3) & 0b111);
        self.operand2 = if I {
            (self.instr >> 6) & 0b111
        } else {
            self.read_reg((self.instr >> 6) & 0b111)
        };

        // ignore extra clock cycles, will be 0
        match (self.instr >> 9) & 1 {
            0 => self.op_add(),
            1 => self.op_sub(),
            _ => 0,
        };

        1
    }

    // ---------- mov, cmp, add, sub- imm8
    #[inline(always)]
    fn execute_thumb_mov_cmp_add_sub_imm8(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb MOV CMP ADD SUB imm8");
        self.operand2 = self.instr & 0b11111111;
        self.reg_dest = (self.instr >> 8) & 0b111;
        // same dest and source reg
        self.operand1 = self.read_reg(self.reg_dest);

        match (self.instr >> 11) & 0b11 {
            0b00 => self.op_mov(),
            0b01 => self.op_cmp(),
            0b10 => self.op_add(),
            0b11 => self.op_sub(),
            _ => 0,
        };

        1
    }

    #[inline(always)]
    fn execute_thumb_alu_general(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb ALU general");
        self.operand2 = self.read_reg((self.instr >> 3) & 0b111);
        self.reg_dest = self.instr & 0b111;
        // same dest and source reg
        self.operand1 = self.read_reg(self.reg_dest);

        let mut shift = false;

        match (self.instr >> 6) & 0b1111 {
            0b0000 => {
                self.op_and();
            }
            0b0001 => {
                self.op_eor();
            }
            0b0101 => {
                self.op_adc();
            }
            0b0110 => {
                self.op_sbc();
            }
            0b1000 => {
                self.op_tst();
            }
            0b1001 => {
                self.operand1 = self.operand2;
                self.operand2 = 0;
                self.op_rsb();
            }
            0b1010 => {
                self.op_cmp();
            }
            0b1011 => {
                self.op_cmn();
            }
            0b1100 => {
                self.op_orr();
            }
            0b1101 => {
                let res = (Wrapping(self.operand1) * Wrapping(self.operand2)).0;
                self.set_flag(Flag::N, res >> 31 > 0);
                self.set_flag(Flag::Z, res == 0);
                self.set_reg(self.reg_dest, res);

                return if self.operand2 >> 8 == 0 || self.operand2 >> 8 == (1 << 24) - 1 {
                    2
                } else if self.operand2 >> 16 == 0 || self.operand2 >> 16 == (1 << 16) - 1 {
                    3
                } else if self.operand2 >> 24 == 0 || self.operand2 >> 24 == (1 << 8) - 1 {
                    4
                } else {
                    5
                };
            }
            0b1110 => {
                self.op_bic();
            }
            0b1111 => {
                self.op_mvn();
            }
            _ => shift = true,
        };
        if !shift {
            return 1;
        }

        self.operand2 &= 0b11111111;
        if self.operand2 > 0 {
            match (self.instr >> 6) & 0b1111 {
                0b0010 => {
                    self.op_thumb_lsl();
                }
                0b0011 => {
                    self.op_thumb_lsr();
                }
                0b0100 => {
                    self.op_thumb_asr();
                }
                0b0111 => {
                    self.op_thumb_ror();
                }

                _ => {}
            };
        } else {
            let res = self.read_reg(self.reg_dest);
            self.set_flag(Flag::N, res >> 31 > 0);
            self.set_flag(Flag::Z, res == 0);
        }

        2
    }

    #[inline(always)]
    fn execute_thumb_hi_bx(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb Hi reg operations or BX");
        self.reg_dest = self.instr & 0b111;
        if (self.instr >> 7) & 1 > 0 {
            self.reg_dest += 8;
        }
        let mut reg_src = (self.instr >> 3) & 0b111;
        if (self.instr >> 6) & 1 > 0 {
            reg_src += 8;
        }
        self.operand1 = self.read_reg(self.reg_dest);
        self.operand2 = self.read_reg(reg_src);

        let clocks = 1 + match (self.instr >> 8) & 0b11 {
            0b00 => {
                self.thumb_modify_flags = false;
                self.op_add()
            }
            0b01 => self.op_cmp(),
            0b10 => {
                self.thumb_modify_flags = false;
                self.op_mov()
            }
            0b11 => {
                if self.operand2 & 1 == 0 {
                    self.set_flag(Flag::T, false);
                    //self.instr_set = InstructionSet::Arm;
                }
                self.actual_pc = (self.operand2 >> 1) << 1;
                //print!(" bx from thumb");
                self.pipeline_instr.clear();
                self.increment_pc = false;
                3
            }
            _ => 0,
        };

        if self.reg_dest == 15 {
            self.actual_pc &= !1;
        }

        clocks
    }

    #[inline(always)]
    fn execute_thumb_pc_relative_load(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb pc relative load");
        let offset = (self.instr & 0b11111111) << 2;
        self.reg_dest = (self.instr >> 8) & 0b111;
        let addr = Wrapping(self.actual_pc) + Wrapping(4) + Wrapping(offset);
        let addr = addr.0;
        self.set_reg(self.reg_dest, bus.read_word(addr as usize & !0b11));
        //print!(" final addr: {:#010x}", addr as usize & !0b11);
        3
    }

    #[inline(always)]
    fn execute_thumb_load_store_reg_offset(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb load/store reg offset");
        let L = (self.instr >> 11) & 1 > 0;
        let B = (self.instr >> 10) & 1 > 0;

        let addr = Wrapping(self.read_reg((self.instr >> 3) & 0b111))
            + Wrapping(self.read_reg((self.instr >> 6) & 0b111));
        let addr = addr.0 as usize;
        self.reg_dest = self.instr & 0b111;

        //self.debug(&format!(" addr: {:#x}, L: {}, store_res: {:#x}, rd: {}", addr, L, self.read_reg(self.reg_dest), self.reg_dest));

        match (L, B) {
            // register -> memory, word
            (false, false) => {
                let res = self.read_reg(self.reg_dest);
                bus.store_word(addr & !(0b11), res);
                2
            }
            // memory -> register, word
            (true, false) => {
                let res = bus.read_word(addr & !(0b11));
                self.set_reg(self.reg_dest, res.rotate_right((addr as u32 & 0b11) << 3));
                3
            }
            // register -> memory, byte
            (false, true) => {
                let res = self.read_reg(self.reg_dest) as u8;
                bus.store_byte(addr, res);
                2
            }
            // memory -> register, byte
            (true, true) => {
                let res = bus.read_byte(addr);
                self.set_reg(self.reg_dest, res as u32);
                3
            }
        }
    }

    #[inline(always)]
    fn execute_thumb_load_store_signed(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb load/store reg signed byte/halfword");
        let H = (self.instr >> 11) & 1 > 0;
        let S = (self.instr >> 10) & 1 > 0;

        let addr = Wrapping(self.read_reg((self.instr >> 3) & 0b111))
            + Wrapping(self.read_reg((self.instr >> 6) & 0b111));
        let addr = addr.0 as usize;
        self.reg_dest = self.instr & 0b111;

        //self.debug(&format!(" addr: {:#x}, H: {}, store_res: {:#x}, rd: {}", addr, H, self.read_reg(self.reg_dest), self.reg_dest));

        match (S, H) {
            // register -> memory, unsigned halfword
            (false, false) => {
                let res = self.read_reg(self.reg_dest) as u16;
                bus.store_halfword(addr, res);
                2
            }
            // memory -> register, unsigned halfword
            (false, true) => {
                let res = (bus.read_halfword(addr & !1) as u32).rotate_right((addr as u32 & 1) * 8);
                self.set_reg(self.reg_dest, res);
                3
            }
            // memory -> register, signed byte
            (true, false) => {
                let mut res = bus.read_byte(addr) as u32;
                if (res >> 7) & 1 > 0 {
                    res |= !0b11111111;
                }
                self.set_reg(self.reg_dest, res);
                3
            }
            // memory -> register, signed halfword
            (true, true) => {
                /*let mut res = bus.read_halfword(addr & !1) as u32;
                if (res >> 15) & 1 > 0{
                    res |= !0b1111111111111111;
                }
                self.set_reg(self.reg_dest, res);*/
                let rotate = (addr as u32 & 1) * 8;
                let mut res = (bus.read_halfword(addr & !1) as u32).rotate_right(rotate);
                if rotate == 0 && (res >> 15) & 1 > 0 {
                    res |= ((1 << 16) - 1) << 16;
                } else if rotate == 8 && (res >> 7) & 1 > 0 {
                    res |= !0b11111111;
                }
                self.set_reg(self.reg_dest, res);
                3
            }
        }
    }

    #[inline(always)]
    fn execute_thumb_load_store_imm5(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb load/store reg imm5");
        let B = (self.instr >> 12) & 1 > 0;
        let L = (self.instr >> 11) & 1 > 0;
        self.reg_dest = self.instr & 0b111;
        let addr = Wrapping(self.read_reg((self.instr >> 3) & 0b111));
        let addr = if B {
            addr + Wrapping((self.instr >> 6) & 0b11111)
        } else {
            addr + Wrapping(((self.instr >> 6) & 0b11111) << 2)
        };
        let addr = addr.0 as usize;

        //self.debug(&format!(" addr: {:#x}, L: {}, B: {}, store_res: {:#x}, rd: {}", addr, L, B, self.read_reg(self.reg_dest), self.reg_dest));

        match (L, B) {
            // register -> memory, word
            (false, false) => {
                let res = self.read_reg(self.reg_dest);
                bus.store_word(addr & (!0b11), res);
                2
            }
            // memory -> register, word
            (true, false) => {
                let res = bus
                    .read_word(addr & (!0b11))
                    .rotate_right((addr as u32 & 0b11) << 3);
                self.set_reg(self.reg_dest, res);
                3
            }
            // register -> memory, byte
            (false, true) => {
                let res = self.read_reg(self.reg_dest) as u8;
                bus.store_byte(addr, res);
                2
            }
            // memory -> register, byte
            (true, true) => {
                let res = bus.read_byte(addr);
                self.set_reg(self.reg_dest, res as u32);
                3
            }
        }
    }

    #[inline(always)]
    fn execute_thumb_load_store_halfword_imm5(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb load/store halfword imm5");
        self.reg_dest = self.instr & 0b111;
        let addr = Wrapping(self.read_reg((self.instr >> 3) & 0b111))
            + Wrapping(((self.instr >> 6) & 0b11111) << 1);
        let rotate = (addr.0 & 1) * 8;
        let addr = addr.0 as usize & !1;

        //self.debug(&format!(" addr: {:#x}, L: {}, H: true, store_res: {:#x}, rd: {}", addr, (self.instr >> 11) & 1 > 0, self.read_reg(self.reg_dest), self.reg_dest));
        //self.debug(&format!(" dma1_src: {:#x}, dma1_dest: {:#x}, dma1_num: {}", bus.read_word(0x40000BC), bus.read_word(0x40000C0), bus.read_halfword(0x40000C4)));
        //self.debug(&format!(" dma2_src: {:#x}, dma2_dest: {:#x}, dma2_num: {}", bus.read_word(0x40000C8), bus.read_word(0x40000CC), bus.read_halfword(0x40000D0)));

        match (self.instr >> 11) & 1 > 0 {
            false => {
                let res = self.read_reg(self.reg_dest) as u16;
                bus.store_halfword(addr, res);
                2
            }
            true => {
                let res = bus.read_halfword(addr);
                self.set_reg(self.reg_dest, (res as u32).rotate_right(rotate));
                3
            }
        }
    }

    // IMPORTANT NOTE: reference materials differ on which register number is SP.
    // cpu_technical_spec_long.pdf says R13. ARM7TDMI_data_sheet.pdf says R7.
    // R13 will be used here. May need to be modified.
    // STR, LDR
    #[inline(always)]
    fn execute_thumb_load_store_sp(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb load/store word sp offset");
        let L = (self.instr >> 11) & 1 > 0;
        let addr = Wrapping(self.read_reg(13)) + Wrapping((self.instr & 0b11111111) << 2);
        let rotate = (addr.0 & 0b11) * 8;
        let addr = addr.0 as usize & !0b11;
        self.reg_dest = (self.instr >> 8) & 0b111;

        //self.debug(&format!(" addr: {:#x}, L: {}, store_res: {:#x}, rd: {}", addr, L, self.read_reg(self.reg_dest), self.reg_dest));

        match L {
            false => {
                let res = self.read_reg(self.reg_dest);
                bus.store_word(addr, res);
                2
            }
            true => {
                let res = bus.read_word(addr);
                self.set_reg(self.reg_dest, res.rotate_right(rotate));
                3
            }
        }
    }

    #[inline(always)]
    fn execute_thumb_load_address(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb load address sp/pc");
        let SP = (self.instr >> 11) & 1 > 0;
        self.reg_dest = (self.instr >> 8) & 0b111;
        let offset = Wrapping((self.instr & 0b11111111) << 2);

        let res = match SP {
            false => Wrapping(self.actual_pc & 0xfffffffc) + Wrapping(4) + offset,
            true => Wrapping(self.read_reg(13)) + offset,
        };
        self.set_reg(self.reg_dest, res.0);

        1
    }

    #[inline(always)]
    fn execute_thumb_sp_offset(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb sp offset");
        let offset = Wrapping((self.instr & 0b1111111) << 2);
        let neg = (self.instr >> 7) & 1 > 0;
        let mut res = Wrapping(self.read_reg(13));
        match neg {
            false => res += offset,
            true => res -= offset,
        };
        self.set_reg(13, res.0);

        1
    }

    #[inline(always)]
    fn execute_thumb_push_pop(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb push/pop");
        let L = (self.instr >> 11) & 1 > 0;
        let R = (self.instr >> 8) & 1 > 0;
        let reg_list = self.instr & 0b11111111;
        let mut cnt = R as u32;
        for i in 0..8 {
            if reg_list & (1 << i) > 0 {
                cnt += 1;
            }
        }
        let mut start_addr = Wrapping(self.read_reg(13));
        if !L {
            start_addr -= Wrapping(4 * cnt);
        }
        let mut addr = start_addr.0 as usize;

        for i in 0..8 {
            if reg_list & (1 << i) > 0 {
                if L {
                    let res = bus.read_word(addr & !0b11);
                    self.set_reg(i, res);
                } else {
                    let res = self.read_reg(i);
                    bus.store_word(addr & !0b11, res);
                }
                addr += 4;
            }
        }
        if R {
            if L {
                let res = bus.read_word(addr);
                self.actual_pc = res & 0xfffffffe;
                self.pipeline_instr.clear();
                self.increment_pc = false;
            } else {
                let res = self.read_reg(14);
                bus.store_word(addr, res);
            }
            addr += 4;
        }

        if L {
            self.set_reg(13, addr as u32);
        } else {
            self.set_reg(13, start_addr.0);
        }

        if L {
            cnt + 2
        } else {
            cnt + 1
        }
    }

    #[inline(always)]
    fn execute_thumb_load_store_multiple(&mut self, bus: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb multiple load/store");
        let reg_list = self.instr & 0b11111111;
        let L = (self.instr >> 11) & 1 > 0;
        let base_reg = (self.instr >> 8) & 0b111;
        let addr = self.read_reg(base_reg);
        let mut addr = addr as usize;

        let mut num_reg = 0;
        for i in 0..8 {
            if reg_list & (1 << i) > 0 {
                num_reg += 1;
                assert!(i != base_reg);
            }
        }
        if num_reg == 0 {
            warn!("critical error: num_reg == 0");
            return 1;
        }
        //assert!(num_reg > 0);

        let mut cnt = 0;
        for i in 0..8 {
            if reg_list & (1 << i) > 0 {
                if !L {
                    let res = self.read_reg(i);
                    bus.store_word(addr & !0b11, res);
                } else {
                    let res = bus.read_word(addr & !0b11);
                    self.set_reg(i, res);
                }
                if cnt == 0 {
                    self.set_reg(base_reg, addr as u32 + num_reg * 4);
                }
                addr += 4;
                cnt += 1;
            }
        }

        if L {
            cnt + 2
        } else {
            cnt + 1
        }
    }

    #[inline(always)]
    fn execute_thumb_cond_branch(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb cond branch");
        if self.check_cond((self.instr >> 8) & 0b1111) {
            let mut offset = (self.instr & 0b11111111) << 1;
            //print!(" offset {:#014b}", offset);
            if (offset >> 8) & 1 > 0 {
                offset |= (!0) << 9;
            }
            let res = Wrapping(self.actual_pc + 4) + Wrapping(offset);
            self.actual_pc = res.0;
            self.pipeline_instr.clear();
            self.increment_pc = false;
            3
        } else {
            1
        }
    }

    #[inline(always)]
    fn execute_thumb_uncond_branch(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb uncond branch");
        let mut offset = (self.instr & 0b11111111111) << 1;
        if (offset >> 11) & 1 > 0 {
            offset |= (!0) << 12;
            //print!(" offset: {:#x}, !0: {:#x}", offset, !0);
        }
        let res = Wrapping(self.reg[Register::R15 as usize]) + Wrapping(offset);
        self.actual_pc = res.0;
        //print!(" actual_pc: {:#x}", self.actual_pc);
        self.pipeline_instr.clear();
        self.increment_pc = false;
        3
    }

    #[inline(always)]
    fn execute_thumb_uncond_branch_link(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb long branch and link");
        let H = (self.instr >> 11) & 1 > 0;
        let mut offset = self.instr & 0b11111111111;

        match H {
            false => {
                offset <<= 12;
                if (offset >> 22) & 1 > 0 {
                    offset |= 0b111111111 << 23;
                }
                let offset = Wrapping(offset) + Wrapping(self.read_reg(Register::R15 as u32));
                self.set_reg(14, offset.0);
            }
            true => {
                let offset = Wrapping(self.read_reg(14)) + Wrapping(offset << 1);
                //print!(" value placed into R15: {:#010x}", offset);
                self.set_reg(14, (self.actual_pc + 2) | 1);
                self.actual_pc = offset.0;
                self.pipeline_instr.clear();
                self.increment_pc = false;
            }
        };

        4
    }

    #[inline(always)]
    fn execute_thumb_software_interrupt(&mut self, _: &mut Bus) -> u32 {
        #[cfg(feature = "debug_instr")]
        self.debug("        thumb SWI");
        self.execute_software_interrupt()
    }

    // ---------- interrupts and halting
    #[inline(always)]
    pub fn halt(&mut self) {
        self.halt = true;
    }

    #[inline(always)]
    pub fn check_interrupt(&self, bus: &Bus) -> bool {
        //!self.read_flag(Flag::I) && // check that interrupt flag is turned off (on means interrupts are disabled)
        bus.read_byte_raw(0x208, MemoryRegion::IO) & 1 == 1 && // check that IME interrupt is turned on
        bus.read_halfword_raw(0x202, MemoryRegion::IO) & bus.read_halfword_raw(0x200, MemoryRegion::IO) > 0
        // check that an interrupt for an active interrupt type has been requested
    }

    // Mode: SVC (supervisor) for software interrupt
    //       IRQ (interrupt) for hardware interrupt
    #[inline(always)]
    fn execute_hardware_interrupt(&mut self) -> u32 {
        //info!("hardware interrupt");
        self.reg[Register::R14_irq as usize] = self.actual_pc + 4;
        let mut cpsr = self.reg[Register::Cpsr as usize];
        self.reg[Register::SPSR_irq as usize] = cpsr;
        self.actual_pc = 0x18;
        self.pipeline_instr.clear();
        self.increment_pc = false;

        // switch to arm
        cpsr &= !(1 << (Flag::T as u32));

        // switch to irq mode
        cpsr &= !0b11111;
        cpsr |= 0b10010;

        //disable interrupt
        cpsr |= 1 << (Flag::I as usize);

        self.set_cpsr(cpsr);

        3
    }

    #[inline(always)]
    fn execute_software_interrupt(&mut self) -> u32 {
        self.reg[Register::R14_svc as usize] = if self.read_flag(Flag::T) {
            self.actual_pc + 2
        } else {
            self.actual_pc + 4
        };
        let mut cpsr = self.reg[Register::Cpsr as usize];
        self.reg[Register::SPSR_svc as usize] = cpsr;
        self.actual_pc = 0x8;
        self.pipeline_instr.clear();
        self.increment_pc = false;

        // switch to arm
        cpsr &= !(1 << (Flag::T as u32));

        // switch to supervisor mode
        cpsr &= !0b11111;
        cpsr |= 0b10011;

        //disable interrupt
        cpsr |= 1 << (Flag::I as usize);

        self.set_cpsr(cpsr);

        3
    }

    // ---------- DMA
    #[inline(always)]
    pub fn check_dma(&self, bus: &Bus) -> bool {
        bus.is_any_dma_active && bus.dma_channels.iter().any(|x| x.check_is_active(bus))
    }

    #[inline(always)]
    pub fn execute_dma(&mut self, bus: &mut Bus) -> u32 {
        let mut res = 0;
        let mut ex1 = false;
        //info!("dma start");
        for i in 0..4 {
            if !bus.dma_channels[i].check_is_active(bus) {
                continue;
            }
            // unsafe in order to prevent unnecessary cloning
            unsafe {
                let ptr = &mut bus.dma_channels[i] as *mut DMA_Channel;
                res += (*ptr).execute_dma(bus);
            }
            ex1 = true;
            // safe code here:
            /*
            let mut dma_channel = bus.dma_channels[i].clone();
            res += dma_channel.execute_dma(bus);
            bus.dma_channels[i] = dma_channel
            */
        }
        //info!("dma end");
        assert!(ex1);
        bus.hblank_dma = false;
        bus.vblank_dma = false;
        bus.set_is_any_dma_active();
        //info!("dma executed");
        res
    }

    // ---------- misc
    #[cfg(feature = "debug_instr")]
    pub fn print_pc(&mut self, bus: &mut Bus) {
        {
            if self.debug_cnt == 0 || !self.bios_end {
                //info!("PC: {:#010x}\n  instr: {:#034b}", self.actual_pc, self.instr);
                return;
            }

            self.debug_cnt -= 1;
            if self.read_flag(Flag::T) {
                info!(
                    "Executing instruction at pc {:#010x}\n   instr: {:#018b} ",
                    self.actual_pc, self.instr
                );
            } else {
                info!(
                    "Executing instruction at pc {:#010x}\n   instr: {:#034b} ",
                    self.actual_pc, self.instr
                );
            }
            info!("    ");
            for i in 0..16 {
                print!("R{}: {:x}, ", i, self.read_reg(i));
            }
            info!();
            info!(
                "N: {}, Z: {}, C: {}, V: {}, CPSR: {:#034b}, IE: {:#018b}, IF: {:#018b}, IME: {}",
                self.read_flag(Flag::N),
                self.read_flag(Flag::Z),
                self.read_flag(Flag::C),
                self.read_flag(Flag::V),
                self.reg[Register::Cpsr as usize],
                bus.read_halfword(0x4000200),
                bus.read_halfword(0x4000202),
                bus.read_byte(0x4000208) & 1
            );
            info!(
                " win_in: {:#018b}, win_out: {:#018b}",
                bus.read_halfword(0x04000048),
                bus.read_halfword(0x0400004a)
            );
            info!();
        }
    }

    #[cfg(feature = "debug_instr")]
    fn debug(&mut self, msg: &str) {
        {
            if self.debug_cnt > 0 && self.bios_end {
                self.debug_cnt -= 1;
                print!("{}", msg);
            }
        }
    }

    // ---------- read and set helpers

    #[inline(always)]
    fn read_pc(&self) -> u32 {
        self.reg[Register::R15 as usize]
    }

    #[inline(always)]
    fn set_pc(&mut self, pc: u32) {
        self.reg[Register::R15 as usize] = pc;
    }

    /*fn read_sp(&self) -> u32 {
        self.reg[Register::R14 as usize]
    }

    fn set_sp(&mut self, sp: u32){
        self.reg[Register::R14 as usize] = sp;
    }*/

    #[inline(always)]
    pub fn read_flag(&self, f: Flag) -> bool {
        let s = f as u32;
        (self.reg[Register::Cpsr as usize] >> s) & 1 > 0
    }

    #[inline(always)]
    fn set_flag(&mut self, f: Flag, val: bool) {
        let s = f as u32;
        if val {
            self.reg[Register::Cpsr as usize] |= 1 << s;
        } else {
            self.reg[Register::Cpsr as usize] &= !(1 << s);
        }
    }

    #[inline(always)]
    fn read_reg(&self, reg: u32) -> u32 {
        let reg = self.reg_map[self.op_mode as usize][reg as usize];
        self.reg[reg as usize]
    }

    #[inline(always)]
    fn set_reg(&mut self, reg: u32, val: u32) {
        let reg = self.reg_map[self.op_mode as usize][reg as usize];
        self.reg[reg as usize] = val;
    }

    #[inline(always)]
    fn set_cpsr(&mut self, val: u32) {
        self.reg[Register::Cpsr as usize] = val;
        self.op_mode = match val & 0b11111 {
            0b10000 => OperatingMode::Usr,
            0b10001 => OperatingMode::Fiq,
            0b10010 => OperatingMode::Irq,
            0b10011 => OperatingMode::Svc,
            0b10111 => OperatingMode::Abt,
            0b11011 => OperatingMode::Und,
            0b11111 => OperatingMode::Sys,
            _ => {
                warn!(
                    "invalid op mode: {}, instr: {:#034b}, pc: {:#x}",
                    val, self.instr, self.actual_pc
                );
                OperatingMode::Sys
            }
        };
    }
}
