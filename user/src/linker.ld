
OUTPUT_ARCH(riscv)
ENTRY(_start)

<<<<<<< HEAD
BASE_ADDRESS = 0x80400000;
=======
BASE_ADDRESS = 0x10000;
>>>>>>> ch4

SECTIONS
{
    . = BASE_ADDRESS;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        *(.bss .bss.*)
        *(.sbss .sbss.*)
<<<<<<< HEAD
        end_bss = .;
=======
>>>>>>> ch4
    }
    /DISCARD/ : {
        *(.eh_frame)
        *(.debug*)
    }
}
