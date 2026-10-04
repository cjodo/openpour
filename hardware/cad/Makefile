OPENSCAD ?= openscad
PARTS = base_tub base_lid head arm_root arm_tip nozzle_holder switch_bracket pump_bracket meter_clip
SRC = openpour.scad config.scad parts.scad vitamins.scad

all: $(PARTS:%=stl/%.stl)

stl/%.stl: $(SRC)
	@mkdir -p stl
	$(OPENSCAD) -D 'part="$*"' -o $@ openpour.scad

preview.png: $(SRC)
	$(OPENSCAD) --imgsize=1400,1100 --camera=40,0,190,62,0,35,900 --colorscheme=Tomorrow -o $@ openpour.scad

clean:
	rm -rf stl preview.png

.PHONY: all clean
