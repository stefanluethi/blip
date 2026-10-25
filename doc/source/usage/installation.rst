Installation
============

What you need depends on what you use:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - To …
     - you need
   * - look at data in the browser
     - nothing: open the hosted web app at
       https://stefanluethi.github.io/blip/, or build and serve it yourself
       (below)
   * - run blip natively
     - Rust, to build it
   * - connect a target, simulate one, record or replay
     - the outpost (Python, uv)
   * - trace from firmware or unit tests
     - sonde (C++17, CMake)

Prerequisites
-------------

* `Rust <https://rustup.rs>`_ |ge| 1.85, with the ``wasm32-unknown-unknown``
  target for the web build::

     rustup target add wasm32-unknown-unknown

* `Trunk <https://trunkrs.dev>`_ to build and serve the web app::

     cargo install --locked trunk

* `uv <https://docs.astral.sh/uv/>`_ for the Python outpost and this
  documentation, which also needs `Graphviz <https://graphviz.org>`_
  (``dot``).
* For sonde: a C++17 compiler and CMake |ge| 3.16 (3.28 or newer keeps
  tinycbor out of the install rules). Cross-compiling for a Cortex-M needs
  ``arm-none-eabi-gcc``.

.. |ge| unicode:: U+2265

blip
----

Native, the binary ends up in ``target/release/blip``::

   cargo build --release -p blip

Web, the output goes to ``blip/dist`` and can be served by any static file
server::

   cd blip && trunk build --release

For development, ``trunk serve`` in ``blip`` builds the web app, serves it on
http://127.0.0.1:8080 and reloads it on changes.

The web app connects to outposts from the browser. Browsers block insecure
``ws://`` connections from a page served over ``https``, like the hosted app,
except to the same machine (``ws://127.0.0.1``, ``ws://localhost``) in most
browsers. To reach an outpost elsewhere on the network, use the native app, a
web app served over plain ``http`` (e.g. ``trunk serve``), or put the outpost
behind ``wss://``.

outpost
-------

The outpost is the Python library ``blip-outpost``. Add it to a project of
your own straight from the repository::

   pip install "blip-outpost @ git+https://github.com/stefanluethi/blip#subdirectory=outpost"

or, with uv and a checkout next to your project,
``uv add --editable ../blip/outpost``. Then ``import blip_outpost`` in your
script and the command ``blip-outpost`` are available, see :doc:`outpost`.

To run the examples and the tests in the repository::

   cd outpost && uv sync

sonde
-----

sonde is built as part of the firmware or test project that uses it:

.. code-block:: cmake

   add_subdirectory(sonde)
   target_link_libraries(firmware PRIVATE sonde::sonde)

To build its tests and examples on their own::

   cd sonde
   cmake -S . -B build
   cmake --build build
   ctest --test-dir build --output-on-failure

See :doc:`sonde` for the CMake options and for cross-compiling.

This documentation
------------------

::

   cd doc
   make html     # output: doc/_build/html/index.html
   make serve    # live-reloading preview

The diagrams are draw.io files in ``doc/source/diagrams``; after editing one,
``make diagrams`` exports the SVGs next to them (needs the draw.io desktop
app on the ``PATH``). ``make screenshots`` renders the screenshots of blip
again, see ``doc/README.md``.
