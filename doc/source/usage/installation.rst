Installation
============

Prerequisites
-------------

* `Rust <https://rustup.rs>`_ |ge| 1.85 with the ``wasm32-unknown-unknown``
  target for the web build::

     rustup target add wasm32-unknown-unknown

* `Trunk <https://trunkrs.dev>`_ to build and serve the web app::

     cargo install --locked trunk

* `uv <https://docs.astral.sh/uv/>`_ for the Python bridge and this
  documentation.

.. |ge| unicode:: U+2265

Building blip
-------------

Native::

   cargo build --release -p blip

Web (output in ``blip/dist``)::

   cd blip && trunk build --release

Installing the bridge
---------------------

::

   cd bridge && uv sync
