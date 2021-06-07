target remote :3333

load
break DefaultHandler
break HardFault
break rust_begin_unwind

continue