# Hardware Design Files

This directory stores board hardware source files and exported reference views.

## Directory Layout

- [easyeda/](easyeda/): EasyEDA project archives.
- [exports/](exports/): exported schematic and PCB reference files.
- [fabrication/](fabrication/): production files such as Gerber, drill, BOM,
  CPL/PnP, and fabrication notes. Keep these separate from human-readable
  exports.

## Files

- [easyeda/sleep-monitor.epro](easyeda/sleep-monitor.epro): EasyEDA project
  archive for the sleep monitor board.
- [exports/sleep-monitor-schematic.svg](exports/sleep-monitor-schematic.svg):
  schematic export for quick inspection.
- [exports/sleep-monitor-schematic.pdf](exports/sleep-monitor-schematic.pdf):
  schematic PDF export for quick inspection and sharing.
- [exports/sleep-monitor-pcb.png](exports/sleep-monitor-pcb.png): PCB view
  export for quick inspection.
- [exports/sleep-monitor-pcb.pdf](exports/sleep-monitor-pcb.pdf): PCB PDF
  export for quick inspection and sharing.
- [fabrication/assembly/sleep-monitor-bom.xlsx](fabrication/assembly/sleep-monitor-bom.xlsx):
  assembly bill of materials.
- [fabrication/assembly/sleep-monitor-pick-and-place.xlsx](fabrication/assembly/sleep-monitor-pick-and-place.xlsx):
  pick-and-place / CPL data.
- [fabrication/gerber-drill/](fabrication/gerber-drill/): neutral Gerber and
  drill layer files for PCB fabrication.

The original vendor-exported Gerber ZIP is not tracked as the canonical
fabrication artifact because it included vendor-specific auxiliary files. The
tracked fabrication directory keeps the neutral Gerber and drill layers
extracted from that archive.

Keep hardware design-file facts synchronized with
[../docs/10-firmware/01-hardware.md](../docs/10-firmware/01-hardware.md).
