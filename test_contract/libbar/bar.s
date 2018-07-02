	.text
	.file	"bar.c"
	.section	.text.add,"",@
	.hidden	add                     # -- Begin function add
	.globl	add
	.type	add,@function
add:                                    # @add
	.param  	i32, i32
	.result 	i32
	.local  	i32, i32, i32, i32, i32, i32
# %bb.0:
	get_global	$push0=, __stack_pointer
	set_local	2, $pop0
	i32.const	$push1=, 16
	set_local	3, $pop1
	get_local	$push4=, 2
	get_local	$push3=, 3
	i32.sub 	$push2=, $pop4, $pop3
	set_local	4, $pop2
	get_local	$push6=, 4
	get_local	$push5=, 0
	i32.store	12($pop6), $pop5
	get_local	$push8=, 4
	get_local	$push7=, 1
	i32.store	8($pop8), $pop7
	get_local	$push10=, 4
	i32.load	$push9=, 12($pop10)
	set_local	5, $pop9
	get_local	$push12=, 4
	i32.load	$push11=, 8($pop12)
	set_local	6, $pop11
	get_local	$push15=, 5
	get_local	$push14=, 6
	i32.add 	$push13=, $pop15, $pop14
	set_local	7, $pop13
	get_local	$push16=, 7
	return  	$pop16
	end_function
.Lfunc_end0:
	.size	add, .Lfunc_end0-add
                                        # -- End function

	.ident	"clang version 7.0.0 (https://git.llvm.org/git/clang.git 00eb2b47bef2f7c89fed207aea90bdc5e53dfecc) (https://git.llvm.org/git/llvm.git/ d4958cbf6c91c714131d60f1ec3a52578e5309f1)"
