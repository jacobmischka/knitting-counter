.PHONY=list flash

list:
	@echo flash

flash: target/avr-atmega328p/debug/knitting-counter.elf
	cargo build
	./uno-runner.sh $<

