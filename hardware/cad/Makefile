OPENSCAD ?= openscad
PARTS = base_tub base_lid platform head arm_root arm_tip nozzle_holder switch_bracket pump_bracket
SRC = openpour.scad config.scad parts.scad vitamins.scad

all: $(PARTS:%=stl/%.stl)

stl/%.stl: $(SRC)
	@mkdir -p stl
	$(OPENSCAD) -D 'part="$*"' -o $@ openpour.scad

preview.png: $(SRC)
	$(OPENSCAD) --render --imgsize=1600,1200 --camera=40,0,190,62,0,35,900 --colorscheme=Tomorrow -o $@ openpour.scad

clean:
	rm -rf stl preview.png

.PHONY: all clean
