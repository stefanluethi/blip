# blip-kit

`blip-kit` keeps its layers: `store` uses no GUI crate and neither `widgets` nor
`workbench`; `widgets` does not use `workbench`. Widgets read data only through the
`Store` trait (`&dyn Store`), with the `Window` they show.
