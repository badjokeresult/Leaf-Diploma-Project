package main

/*
#cgo LDFLAGS: -L/home/glomonosov/projs/Leaf-Diploma-Project/target/release -lleaf -R/home/glomonosov/projs/Leaf-Diploma-Project/target/release

#include <stdlib.h>
#include <stdio.h>
void hello(const char*);
*/
import "C"
import "unsafe"

func main() {
	message := "Hello from Rust"
	cmessage := C.CString(message)
	defer C.free(unsafe.Pointer(cmessage))

	C.hello(cmessage)
}
