.PHONY: clean
clean:
	find . -type d -name 'target' -mindepth 2 -maxdepth 2 -exec bash -c 'echo rm -fr "{}" && rm -fr "{}"' ';'
