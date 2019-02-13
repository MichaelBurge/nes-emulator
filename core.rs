struct cpu {
    acc: u8,
    x: u8,
    y: u8,
    pc: u16,
    sp: u8,
    carry: bool,
    zero: bool,
    interruptd: bool,
    decimal: bool,
    overflow: bool,
    negative: bool,
}

enum operation {
    ADC, AND, ASL, BCC,
    BCS, BEQ, BIT, BMI,
    BNE, BPL, BRK, BVC,
    BVS, CLC, CLD, CLI,
    CLV, CMP, CPX, CPY,
    DEC, DEX, DEY, EOR,
    INC, INX, INY, JMP,
    JSR, LDA, LDX, LDY,
    LSR, NOP, ORA, PHA,
    PHP, PLA, PLP, ROL,
    ROR, RTI, RTS, SBC,
    SEC, SED, SEI, STA,
    STX, STY, TAX, TAY,
    TSX, TXA, TXS, TYA,
    // "Extra" opcodes
    KIL,
}

enum addressing_mode {
    immediate,
    zero_page, zero_page_x,
    absolute, absolute_x, absolute_y,
    indirect, indirect_x, indirect_y,
    relative,
    accumulator,
    implicit,
}

const STACK_PAGE = 0x0100u16;

type cycle_count = u8;
type memory = [u8; 65536];

//
const abs = absolute;
const acc = accumulator;
const imm = immediate;
const imp = implicit;
const izx = indirect_x;
const zp  = zero_page;
const zpx = zero_page_x;
const rel = relative;
const abx = absolute_x;
const aby = absolute_y;

// Opcode table: http://www.oxyron.de/html/opcodes02.html
const opcode_table: [(operation, addressing_mode, cycle_count, cycle_count)] =
    // TODO Audit each record to see that it was input correctly
    // (Operation, addressing mode, clock cycles, extra clock cycles if page boundary crossed)
    [   // 0x
        (BRK, imp, 7, 0), // x0
        (ORA, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (SLO, izx, 8, 0), // x3
        (NOP, zp,  3, 0), // x4
        (ORA, zp,  3, 0), // x5
        (ASL, zp,  5, 0), // x6
        (SLO, zp,  5, 0), // x7
        (PHP, imp, 3, 0), // x8
        (ORA, imm, 2, 0), // x9
        (ASL, acc, 2, 0), // xA
        (ANC, imm, 2, 0), // xB
        (NOP, abs, 4, 0), // xC
        (ORA, abs, 4, 0), // xD
        (ASL, abs, 6, 0), // xE
        (SLO, abs, 6, 0), // xF
        // 1x
        (BPL, rel, 2, 1), // x0
        (ORA, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (SLO, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (ORA, zpx, 4, 0), // x5
        (ASL, zpx, 6, 0), // x6
        (SLO, zpx, 6, 0), // x7
        (CLC, imp, 2, 0), // x8
        (ORA, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (SLO, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (ORA, abx, 4, 1), // xD
        (ASL, abx, 7, 0), // xE
        (SLO, abx, 7, 0), // xF
        // 2x
        (JSR, abs, 6, 0), // x0
        (AND, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (RLA, izx, 8, 0), // x3
        (BIT, zp,  3, 0), // x4
        (AND, zp,  3, 0), // x5
        (ROL, zp,  5, 0), // x6
        (RLA, zp,  5, 0), // x7
        (PLP, imp, 4, 0), // x8
        (AND, imm, 2, 0), // x9
        (ROL, acc, 2, 0), // xA
        (ANC, imm, 2, 0), // xB
        (BIT, abs, 4, 0), // xC
        (AND, abs, 4, 0), // xD
        (ROL, abs, 6, 0), // xE
        (RLA, abs, 6, 0), // xF
        // 3x
        (BMI, rel, 2, 1), // x0
        (AND, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (RLA, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (AND, zpx, 4, 0), // x5
        (ROL, zpx, 6, 0), // x6
        (RLA, zpx, 6, 0), // x7
        (SEC, imp, 2, 0), // x8
        (AND, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (RLA, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (AND, abx, 4, 1), // xD
        (ROL, abx, 7, 0), // xE
        (RLA, abx, 7, 0), // xF
        // 4x
        (RTI, imp, 6, 0), // x0
        (EOR, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (SRE, izx, 8, 0), // x3
        (NOP, zp,  3, 0), // x4
        (EOR, zp,  3, 0), // x5
        (LSR, zp,  5, 0), // x6
        (SRE, zp,  5, 0), // x7
        (PHA, imp, 3, 0), // x8
        (EOR, imm, 2, 0), // x9
        (LSR, imp, 2, 0), // xA
        (ALR, imm, 2, 0), // xB
        (JMP, abs, 3, 0), // xC
        (EOR, abs, 4, 0), // xD
        (LSR, abs, 6, 0), // xE
        (SRE, abs, 6, 0), // xF
        // 5x
        (BVC, rel, 2, 1), // x0
        (EOR, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (SRE, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (EOR, zpx, 4, 0), // x5
        (LSR, zpx, 6, 0), // x6
        (SRE, zpx, 6, 0), // x7
        (CLI, imp, 2, 0), // x8
        (EOR, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (SRE, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (EOR, abx, 4, 1), // xD
        (LSR, abx, 7, 0), // xE
        (SRE, abx, 7, 0), // xF
        // 6x
        (RTS, imp, 6, 0), // x0
        (ADC, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (RRA, izx, 8, 0), // x3
        (NOP, zp,  3, 0), // x4
        (ADC, zp,  3, 0), // x5
        (ROR, zp,  5, 0), // x6
        (RRA, zp,  5, 0), // x7
        (PLA, imp, 4, 0), // x8
        (ADC, imm, 2, 0), // x9
        (ROR, imp, 2, 0), // xA
        (ARR, imm, 2, 0), // xB
        (JMP, ind, 5, 0), // xC
        (ADC, abs, 4, 0), // xD
        (ROR, abs, 6, 0), // xE
        (RRA, abs, 6, 0), // xF
        // 7x
        (BVS, rel, 2, 1), // x0
        (ADC, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (RRA, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (ADC, zpx, 4, 0), // x5
        (ROR, zpx, 6, 0), // x6
        (RRA, zpx, 6, 0), // x7
        (SEI, imp, 2, 0), // x8
        (ADC, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (RRA, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (ADC, abx, 4, 1), // xD
        (ROR, abx, 7, 0), // xE
        (RRA, abx, 7, 0), // xF
        // 8x
        (NOP, imm, 2, 0), // x0
        (STA, izx, 6, 0), // x1
        (NOP, imm, 2, 0), // x2
        (SAX, izx, 6, 0), // x3
        (STY, zp,  3, 0), // x4
        (STA, zp,  3, 0), // x5
        (STX, zp,  3, 0), // x6
        (SAX, zp,  3, 0), // x7
        (DEY, imp, 2, 0), // x8
        (NOP, imm, 2, 0), // x9
        (TXA, imp, 2, 0), // xA
        (XAA, imm, 2, 1), // xB
        (STY, abs, 4, 0), // xC
        (STA, abs, 4, 0), // xD
        (STX, abs, 4, 0), // xE
        (SAX, abs, 4, 0), // xF
        // 9x
        (BCC, rel, 2, 1), // x0
        (STA, izy, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (AHX, izy, 6, 0), // x3
        (STY, zpx, 4, 0), // x4
        (STA, zpx, 4, 0), // x5
        (STX, zpy, 4, 0), // x6
        (SAX, zpy, 4, 0), // x7
        (TYA, imp, 2, 0), // x8
        (STA, aby, 5, 0), // x9
        (TXS, imp, 2, 0), // xA
        (TAS, aby, 5, 0), // xB
        (SHY, abx, 5, 0), // xC
        (STA, abx, 5, 0), // xD
        (SHX, aby, 5, 0), // xE
        (AHX, aby, 5, 0), // xF
        // Ax
        (LDY, imm, 2, 0), // x0
        (LDA, izx, 6, 0), // x1
        (LDX, imm, 2, 0), // x2
        (LAX, izx, 6, 0), // x3
        (LDY, zp,  3, 0), // x4
        (LDA, zp,  3, 0), // x5
        (LDX, zp,  3, 0), // x6
        (LAX, zp,  3, 0), // x7
        (TAY, imp, 2, 0), // x8
        (LDA, imm, 2, 0), // x9
        (TAX, imp, 2, 0), // xA
        (LAX, imm, 2, 0), // xB
        (LDY, abs, 4, 0), // xC
        (LDA, abs, 4, 0), // xD
        (LDX, abs, 4, 0), // xE
        (LAX, abs, 4, 0), // xF
        // Bx
        (BCS, rel, 2, 1), // x0
        (LDA, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (LAX, izy, 5, 1), // x3
        (LDY, zpx, 4, 0), // x4
        (LDA, zpx, 4, 0), // x5
        (LDX, zpy, 4, 0), // x6
        (LAX, zpy, 4, 0), // x7
        (CLV, imp, 2, 0), // x8
        (LDA, aby, 4, 1), // x9
        (TSX, imp, 2, 0), // xA
        (LAS, aby, 4, 1), // xB
        (LDY, abx, 4, 1), // xC
        (LDA, abx, 4, 1), // xD
        (LDX, aby, 4, 1), // xE
        (LAX, aby, 4, 1), // xF
        // Cx
        (CPY, imm, 2, 0), // x0
        (CMP, izx, 6, 0), // x1
        (NOP, imm, 2, 0), // x2
        (DCP, izx, 8, 0), // x3
        (CPY, zp,  3, 0), // x4
        (CMP, zp,  3, 0), // x5
        (DEC, zp,  5, 0), // x6
        (DCP, zp,  5, 0), // x7
        (INY, imp, 2, 0), // x8
        (CMP, imm, 2, 0), // x9
        (DEX, imp, 2, 0), // xA
        (AXS, imm, 2, 0), // xB
        (CPY, abs, 4, 0), // xC
        (CMP, abs, 4, 0), // xD
        (DEC, abs, 6, 0), // xE
        (DCP, abs, 6, 0), // xF
        // Dx
        (BNE, rel, 2, 1), // x0
        (CMP, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (DCP, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (CMP, zpx, 4, 0), // x5
        (DEC, zpx, 6, 0), // x6
        (DCP, zpx, 6, 0), // x7
        (CLD, imp, 2, 0), // x8
        (CMP, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (DCP, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (CMP, abx, 4, 1), // xD
        (DEC, abx, 7, 0), // xE
        (DCP, abx, 7, 0), // xF
        // Ex
        (CPX, imm, 2, 0), // x0
        (SBC, izx, 6, 0), // x1
        (NOP, imm, 2, 0), // x2
        (ISC, izx, 8, 0), // x3
        (CPX, zp,  3, 0), // x4
        (SBC, zp,  3, 0), // x5
        (INC, zp,  5, 0), // x6
        (ISC, zp,  5, 0), // x7
        (INX, imp, 2, 0), // x8
        (SBC, imm, 2, 0), // x9
        (NOP, imp, 2, 0), // xA
        (SBC, imm, 2, 0), // xB
        (CPX, abs, 4, 0), // xC
        (SBC, abs, 4, 0), // xD
        (INC, abs, 6, 0), // xE
        (ISC, abs, 6, 0), // xF
        // Fx
        (BEQ, rel, 2, 1), // x0
        (SBC, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (ISC, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (SBC, zpx, 4, 0), // x5
        (INC, npx, 6, 0), // x6
        (ISC, zpx, 6, 0), // x7
        (SED, imp, 2, 0), // x8
        (SBC, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (ISC, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (SBC, abx, 4, 1), // xD
        (INC, abx, 7, 0), // xE
        (ISC, abx, 7, 0), // xF
        ];

struct instruction {
    op: operation,
    mode: addressing_mode,
    mode_args: u16,
}

fn clock(c: &cpu, m: &memory) {
    let i = decode_instruction(c, m);
    execute_instruction(c, i, m);
}


fn decode_instruction(c: &cpu, m: &memory) -> instruction {
    let ptr = c.pc;
    let opcode = m[ptr];
    let (op, mode, clocks, page_clocks) = opcode_table[opcode];
    let mode_args = decode_addressing_mode(mode, ptr+1, m);
    return instruction(op, mode, mode_args);
}

fn execute_instruction(c: &cpu, m: &memory) {
    let v = decode_addressing_mode(i.addressing_mode, c.pc+1, c, m);
    let write_target = match mode {
        accumulator => &c.acc,
        _           => &memory[v],
    };

    match op {
        ADC => { execute_adc(v, c) },
        AND => { execute_and(v, c) },
        ASL => { execute_asl(v, c) },
        BCC => { execute_bcc(v, c) },
        BCS => { execute_bcs(v, c) },
        BEQ => { execute_beq(v, c) },
        BIT => { execute_bit(v, c) },
        BMI => { execute_bmi(v, c) },
        BNE => { execute_bne(v, c) },
        BPL => { execute_bpl(v, c) },
        BRK => { execute_brk(c) },
        BVC => { execute_bvc(v, c) },
        CLC => { execute_clc(c) },
        CLD => { execute_cld(c) },
        CLI => { execute_cli(c) },
        CLV => { execute_clv(c) },
        CMP => { execute_cmp(v, c) },
        CPX => { execute_cpx(v, c) },
        CPY => { execute_cpy(v, c) },
        DEC => { execute_dec(write_target, c) },
        DEX => { execute_dex(c) },
        DEY => { execute_dey(c) },
        EOR => { execute_eor(v, c) },
        INC => { execute_inc(write_target, c) },
        INX => { execute_inx(c) },
        INY => { execute_inx(c) },
        JMP => { execute_jmp(v, c) },
        JSR => { execute_jsr(v, c, m) },
        LDA => { execute_lda(v, c) },
        LDX => { execute_ldx(v, c) },
        LDY => { execute_ldy(v, c) },
        LSR => { execute_lsr(write_target) },
        NOP => { execute_nop() },
        ORA => { execute_ora(v, c) },
        PHA => { execute_pha(c, m) },
        PHP => { execute_php(c, m) },
        PLA => { execute_pla(c, m) },
        PLP => { execute_plp(c, m) },
        ROL => { execute_rol(write_target, c) },
        ROR => { execute_ror(write_target, c) },
        RTI => { execute_rti(c, m) },
        RTS => { execute_rts(c, m) },
        SBC => { execute_sbc(c, m) },
        SEC => { execute_sec(c) },
        SED => { execute_sed(c) },
        SEI => { execute_sei(c) },
        STA => { execute_sta(write_target, c) },
        STX => { execute_stx(write_target, c) },
        STY => { execute_sty(write_target, c) },
        TAX => { execute_tax(c) },
        TAY => { execute_tay(c) },
        TSX => { execute_tsx(c) },
        TXA => { execute_txa(c) },
        TXS => { execute_txs(c) },
        TYA => { execute_tya(c) },
        KIL => { panic!("KIL instruction encountered") },
    }
}

fn decode_addressing_mode(mode: addressing_mode, ptr: u16, c: cpu, m: &memory) -> u16 {
    match mode {
        immediate   => peek(ptr, m),
        zero_page   => peek(peek(ptr, m), m),
        zero_page_x => peek_offset(peek(ptr, m), c.x),
        absolute    => peek16(ptr),
        absolute_x  => peek_offset16(ptr, c.x, m),
        absolute_y  => peek_offset16(ptr, c.y, m),
        indirect    => peek16(peek16(ptr, m), m),
        indirect_x  => peek16(peek_offset(ptr, c.x, m), m),
        indirect_y  => peek16(peek_offset(ptr, c.y, m), m),
        relative    => peek(ptr, m),
        accumulator => 0xDEAD,
        implicit    => 0xDEAD,
    }
}

// BEGIN instructions

fn execute_adc(v: u8, c: &cpu) {
    let (x1, o1) = overflowing_add(v, c.acc);
    let (x2, o2) = overflowing_add(x1, c.carry as u8);
    c.carry = o1 | o2;
    c.acc = x2;
    update_accumulator_flags(c);
}

fn execute_and(v: u8, c: &cpu) {
    c.acc &= v;
    update_accumulator_flags(c);
}

fn execute_asl(v: u8, c: &cpu) {
    let (x, o) = overflowing_shl(v, c.acc);
    c.carry = o;
    c.acc = x;
    update_accumulator_flags(c);
}

fn execute_branch(v: u8, c: &cpu) {
    c.pc += (v as i8);
}

fn execute_bcc(v: u8, c: &cpu) {
    if !c.carry
    { execute_branch(v, c); }
}

fn execute_bcs(v: u8, c: &cpu) {
    if c.carry
    { execute_branch(v, c); }
}

fn execute_beq(v: u8, c: &cpu) {
    if c.zero
    { execute_branch(v, c); }
}

fn execute_bit(v: u8, c: &cpu) {
    let x = v & c.acc;
    c.negative = 0b10000000 & x as bool;
    c.overflow = 0b01000000 & x as bool;
    c.zero = (x == 0);
}

fn execute_bmi(v: u8, c: &cpu) {
    if c.negative
    { execute_branch(v, c); }
}

fn execute_bne(v: u8, c: &cpu) {
    if !c.zero
    { execute_branch(v, c); }
}

fn execute_bpl(v: u8, c: &cpu) {
    if !c.negative
    { execute_branch(v, c); }
}

fn execute_brk(c: &cpu, m: &memory) {
    push_stack16(c.pc);
    push_stack(status_register_byte(true, c), c, m);
    c.pc = peek16(0xFFFE, m);
}

fn execute_bvc(v: u8, c: &cpu) {
    if !c.overflow
    { execute_branch(v, c); }
}

fn execute_bvs(v: u8, c: &cpu) {
    if c.overflow
    { execute_branch(v, c); }
}

fn execute_clc(c: &cpu) {
    c.carry = false;
}

fn execute_cld(c: &cpu) {
    c.decimal = false;
}

fn execute_cli(c: &cpu) {
    c.interrupt_disable = false;
}

fn execute_clv(c: &cpu) {
    c.overflow = false;
}

fn execute_compare(v1: u8, v2: u8, c: &cpu) {
    let result = wrapping_sub(v1, v2);
    c.carry = (result >= 0);
    c.zero = (result == 0);
    c.negative = is_negative(result);
}

fn execute_cmp(v: u8, c: &cpu) {
    execute_compare(c.acc, v, c);
}

fn execute_cpx(v: u8, c: &cpu) {
    execute_compare(c.x, v, c);
}

fn execute_cpy(v: u8, c: &cpu) {
    execute_compare(c.y, v, c);
}

fn execute_dec(v: u8&, c: &cpu) {
    *v = wrapping_sub(*v, 1);
    update_result_flags(*v);
}

fn execute_dex(c: &cpu) {
    execute_dec(c.x, c);
}

fn execute_dey(c: &cpu) {
    execute_dec(c.y, c);
}

fn execute_eor(v: u8, c: &cpu) {
    c.acc ^= v;
    update_accumulator_flags(c);
}

fn execute_inc(v: u8&, c: &cpu) {
    wrapping_add(*v, 1);
    update_result_flags(*v, c);
}

fn execute_inx(c: &cpu) {
    execute_inc(c.x, c);
}

fn execute_iny(c: &cpu) {
    execute_inc(c.y, c);
}

fn execute_jmp(ptr: u16, c: &cpu) {
    c.pc = ptr;
}

fn execute_jsr(ptr: u16, c: &cpu, m: &memory) {
    push_stack(c.pc, c, m);
    c.pc = ptr;
}

fn execute_lda(v: u8, c: &cpu) {
    c.acc = v;
    update_accumulator_flags(c);
}

fn execute_ldx(v: u8, c: &cpu) {
    c.x = v;
    update_result_flags(c.x);
}

fn execute_ldy(v: u8, c: &cpu) {
    c.y = v;
    update_result_flags(c.y);
}

fn execute_lsr(v: &u8, c: &cpu) {
    c.carry = v & 0b00000001 as bool;
    v = wrapping_shr(v, 1);
    update_result_flags(v, c);
}

fn execute_nop() { }

fn execute_ora(v: u8, c: &cpu) {
    c.acc |= v;
    update_accumulator_flags(c);
}

fn execute_pha(c: &cpu, m: &memory) {
    push_stack(c.acc, c, m);
}

fn execute_php(c: &cpu, m: &memory) {
    push_stack(status_register_byte(true, c), c, m);
}

fn execute_pla(c: &cpu, m: &memory) {
    c.acc = pop_stack(c, m);
    update_accumulator_flags(c);
}

fn execute_plp(c: &cpu, m: &memory) {
    set_status_register_from_byte(pop_stack(c, m));
}

fn execute_rol(v: &u8, c: &cpu) {
    c.carry = v & (1 << 7) as bool;
    v = rotate_left(v, 1);
    update_result_flags(v, c);
}

fn execute_ror(v: &u8, c: &cpu) {
    c.carry = v & (1 << 0) as bool;
    v = rotate_right(v, 1);
    update_result_flags(v, c);
}

fn execute_rti(c: &cpu, m: &memory) {
    set_status_register_from_byte(pop_stack(c, m), c);
    c.pc = pop_stack16(c, m);
}

fn execute_rts(c: &cpu, m: &memory) {
    c.pc = pop_stack16(c, m);
}

fn execute_sbc(c: &cpu, m: &memory) {
    let (x1, o1) = overflowing_sub(c.acc, v);
    let (x2, o2) = overflowing_sub(x1, !c.carry as u8);
    c.carry = o1 | o2;
    c.acc = x2;
    update_accumulator_flags(c);
}

fn execute_sec(c: &cpu) {
    c.carry = true;
}

fn execute_sed(c: &cpu) {
    c.decimal = true;
}

fn execute_sei(c: &cpu) {
    c.interruptd = true;
}

fn execute_sta(v: &u8, c: &cpu) {
    *v = c.acc;
}

fn execute_stx(v: &u8, c: &cpu) {
    *v = c.x;
}

fn execute_sty(v: &u8, c: &cpu) {
    *v = c.y;
}

fn execute_tax(c: &cpu) {
    c.x = c.acc;
    update_result_flags(c.x);
}

fn execute_tay(c: &cpu) {
    c.y = c.acc;
    update_result_flags(c.y);
}

fn execute_tsx(c: &cpu) {
    c.x = c.sp;
    update_result_flags(c.x);
}

fn execute_txa(c: &cpu) {
    c.acc = c.x;
    update_accumulator_flags(c);
}

fn execute_txs(c: &cpu) {
    c.sp = c.x;
}

fn execute_tya(c: &cpu) {
    c.acc = c.y;
    update_accumulator_flags(c);
}

// END instructions

fn lea(ptr: u16, os: i16) -> u16 {
    return wrapped_add(ptr, os as u16);
}

fn push_stack(v: u8, c: &cpu, m: &memory) {
    poke_offset(STACK_PAGE, c.sp, m);
    c.sp = lea(c.sp, -1);
}

fn peek_stack(c: &cpu, m: &memory) {
    peek_offset(STACK_PAGE, lea(c.sp, 1), m);
}

fn pop_stack(c: &cpu, m: &memory) {
    c.sp = lea(c.sp, 1);
    return peek_offset(STACK_PAGE, c.sp, m);
}

fn push_stack16(v: u16, c: &cpu, m: &memory) {
    push_stack(v & 0xFF, c, m);
    push_stack(v & 0xFF00 >> 8, c, m);
}

fn pop_stack16(c: &cpu, m: &memory) {
    let msb = pop_stack(c, m);
    let lsb = pop_stack(c, m);
    return msb << 8 + lsb;
}

fn peek(ptr: u16, m: &memory) -> u8 {
    return m[ptr];
}

fn peek16(ptr: u16, m: &memory) -> u16 {
    return peek(ptr, m) + peek(lea(ptr, 1), m) << 8;
}

fn peek_offset(ptr: u16, os: i16, m: &memory) -> u8 {
    return peek(lea(ptr, os), m);
}

fn peek_offset16(ptr: u16, os: i16, m: &memory) -> u16 {
    return peek16(lea(ptr, os), m);
}

fn poke(ptr: u16, v: u8, m: &memory) {
    m[ptr] = v;
}

fn poke_offset(ptr: u16, os: i16, v: u8, m: &memory) {
    poke(lea(ptr, os), v, m);
}

fn update_result_flags(v: u8, c: &cpu) {
    c.zero = (v == 0);
    c.negative = is_negative(v);
}

fn update_accumulator_flags(c: &cpu) {
    update_result_flags(c.acc, c);
}

fn is_negative(v: u8) -> bool {
    return (v >= 128);
}

fn status_register_byte(is_instruction: bool, c: &cpu) -> u8 {
    let result =
        (c.carry      as u8) << 0 +
        (c.zero       as u8) << 1 +
        (c.interruptd as u8) << 2 +
        (c.decimal    as u8) << 3 +
        0                    << 4 + // Break flag
        1                    << 5 +
        (c.overflow   as u8) << 6 +
        (c.negative   as u8) << 7;
    return result;
}

fn set_status_register_from_byte(v: u8, c: &cpu) {
    c.carry      = v & 0b00000001 as bool;
    c.zero       = v & 0b00000010 as bool;
    c.interruptd = v & 0b00000100 as bool;
    c.decimal    = v & 0b00001000 as bool;
    // Break isn't a real register
    // Bit 5 is unused
    c.overflow   = v & 0b01000000 as bool;
    c.negative   = v & 0b10000000 as bool;
}
