	.text
	.file	"bar.c"
	.section	.text.add,"",@
	.hidden	add                     # -- Begin function add
	.globl	add
	.type	add,@function
add:                                    # @add
	.param  	i32, i32
	.result 	i32
	.local  	i32
# %bb.0:
	get_global	$push3=, __stack_pointer
	i32.const	$push4=, 16
	i32.sub 	$push6=, $pop3, $pop4
	tee_local	$push5=, 2, $pop6
	get_local	$push7=, 0
	i32.store	12($pop5), $pop7
	get_local	$push9=, 2
	get_local	$push8=, 1
	i32.store	8($pop9), $pop8
	get_local	$push10=, 2
	i32.load	$push2=, 12($pop10)
	get_local	$push11=, 2
	i32.load	$push1=, 8($pop11)
	i32.add 	$push0=, $pop2, $pop1
                                        # fallthrough-return: $pop0
	end_function
.Lfunc_end0:
	.size	add, .Lfunc_end0-add
                                        # -- End function

	.ident	"clang version 7.0.0 (https://git.llvm.org/git/clang.git 00eb2b47bef2f7c89fed207aea90bdc5e53dfecc) (https://git.llvm.org/git/llvm.git/ d4958cbf6c91c714131d60f1ec3a52578e5309f1)"
