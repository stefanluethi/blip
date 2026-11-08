Writing Requirements
====================

Need types
----------

.. list-table::
   :header-rows: 1
   :widths: 15 15 70

   * - Directive
     - ID prefix
     - Use for
   * - ``stk``
     - ``STK_``
     - Stakeholder needs: *why* something is wanted, from a user's view.
   * - ``req``
     - ``REQ_``
     - System/software requirements: *what* the software shall do.
   * - ``arch``
     - ``ARCH_``
     - Architecture elements (components) that implement requirements.
   * - ``test``
     - ``TEST_``
     - Test cases that verify requirements.

IDs are mandatory and must match ``^(STK|REQ|ARCH|TEST)_[A-Z0-9_]+$``.

Where items go
--------------

Stakeholder needs are in :doc:`stakeholder` and architecture elements in
:doc:`architecture`. Requirements have one page per area in ``system/`` (and
``documentation/`` for the documentation), tests one page per area in
``tests/``, named like the requirements page of the first requirement each
verifies. A new area gets a page in ``system/`` and in ``tests/``, listed in the
toctree of both ``index.rst`` files. Keep a page to a few hundred lines; split
one that grows past that by topic.

Options and links
-----------------

``:status:``
   One of ``draft``, ``approved``, ``implemented``, ``verified``,
   ``rejected``.
``:component:``
   Affected component(s): ``blip``, ``proto``, ``outpost``, ``sonde``,
   ``doc`` (the user documentation).
``:priority:``
   ``high``, ``medium`` or ``low``.
``:rationale:``
   Optional justification.
``:satisfies:``
   ``req`` |rarr| ``stk``, ``arch`` |rarr| ``req``.
``:verifies:``
   ``test`` |rarr| ``req``.
``:implements:``
   Reserved for linking code-level items to ``arch`` elements.

.. |rarr| unicode:: U+2192

Example
-------

.. code-block:: rst

   .. req:: Export plot data
      :id: REQ_VIS_EXPORT
      :status: draft
      :component: blip
      :priority: low
      :satisfies: STK_LIVE_OBSERVE

      blip shall export the visible plot data as CSV.

The configuration lives in ``doc/source/conf.py`` (``needs_types``,
``needs_extra_links`` …). Every build also writes ``needs.json`` to the build
directory for use by external tools.
