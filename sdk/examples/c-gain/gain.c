/* A kabl module in plain C: one audio in, one audio out, a `gain` knob. 0BSD.
 *
 * The ABI is language-neutral (docs/sdk/ABI.md). Only Rust has a crate and a template; this page
 * shows the whole contract in C. No libc, no imports, no allocation: every buffer is a static. */

#define MAX_BLOCK 2048 /* the host needs slots of at least 2048 frames */

/* Metadata: a wasm custom section named `kabl.meta`, one line per item. C cannot make a custom
 * section from a variable (clang puts it in the data section), so the assembler does it:
 * `.section .custom_section.NAME,"",@` opens a custom section called NAME and `.ascii` writes its
 * bytes. */
#define LINE(s) ".ascii \"" s "\\n\"\n"
__asm__(".section .custom_section.kabl.meta,\"\",@\n"
        LINE("kind example.cgain")
        LINE("version 1")
        LINE("abi_minor 1")
        LINE("name C Gain")
        LINE("explain Turns the signal up or down (written in C).")
        LINE("category utility")
        LINE("rate voice")
        LINE("width 4")
        LINE("port in audio in 1")
        LINE("port out audio out 1")
        LINE("param gain 0 4 1 linear 5 x")
        ".text\n");

/* The buffers the host reads and writes. Zero-initialised statics live in .bss: they cost no
 * blob bytes. Slots are MAX_BLOCK floats each, params are one float per `param` line. */
static float input[MAX_BLOCK];
static float output[MAX_BLOCK];
static float params[1];

/* The layout record `init` returns: nine u32 words (ABI.md section 3). */
static unsigned layout[9];

__attribute__((export_name("init"))) int init(float sample_rate, int max_block) {
    (void)sample_rate;
    (void)max_block;
    layout[0] = (unsigned)(unsigned long)input;  /* input */
    layout[1] = (unsigned)(unsigned long)output; /* output */
    layout[2] = (unsigned)(unsigned long)params; /* params */
    layout[3] = 0;                               /* state: none */
    layout[4] = 0;                               /* state_len */
    layout[5] = MAX_BLOCK;                       /* max_block */
    layout[6] = 1;                               /* in_slots: must match the `port in` lines */
    layout[7] = 1;                               /* out_slots */
    layout[8] = 1;                               /* n_params */
    return (int)(unsigned long)layout;
}

__attribute__((export_name("process"))) void process(int n_frames) {
    float g = params[0];
    for (int i = 0; i < n_frames; i++) output[i] = input[i] * g;
}

__attribute__((export_name("reset"))) void reset(void) {}
