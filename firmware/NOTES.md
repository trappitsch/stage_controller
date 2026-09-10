# Firmware development notes

## TMC2209

[Datasheet Rev v1.09](https://www.analog.com/media/en/technical-documentation/data-sheets/TMC2209_datasheet_rev1.09.pdf):
Section 1 introduction describes the possible interfaces.
We should probably select option 3,
"STEP/DIR Driver with Full Diagnostics and Control".
This interface only needs the `STEP`, `DIR`, and `PDN_UART` pins connected.
All configurations are written over UART,
diagnostics is obtained through that interface as well.
`STEP`/`DIR` interface will be used for precise stepping,
however, we can still use the internal pulse generator (if we want to)
for "movement until velocity is set to zero again".

**Emergency stop:**: If we leave the `nEN` pin connected to a GPIO,
we can initialize this output at `Low` in the beginning.
If an end switch fires and we need to interrupt the movement,
this can be achieved by throwing high on the `nEN`,
which will de-energize the motor and thus stop it.
Note that this de-energization will leave the axis manually movable.
However, end switches should generally not be triggered,
so this should be acceptable.
