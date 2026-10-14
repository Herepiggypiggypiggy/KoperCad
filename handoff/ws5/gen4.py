# Fourth batch: terminal strip accessories.
emit("terminals/end_plate", "Terminal end plate",
     "End plate closing the open side of the last terminal on a strip.",
     "accessory.terminal.end_plate", ["end plate", "end cover", "terminal", "accessory"],
     [], [TAG_LEFT(-3, 4)], [R([-1, 0], [2, 8], fill=True)], {"kind": "none"})
emit("terminals/end_bracket", "Terminal end bracket",
     "End bracket (end stop) clamping a terminal strip on the DIN rail.",
     "accessory.terminal.end_bracket", ["end bracket", "end stop", "end clamp", "terminal", "accessory"],
     [], [TAG_LEFT(-4, 4)], [R([-2, 0], [4, 8]), L([-2, 8], [2, 0])], {"kind": "none"})
