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

Options and links
-----------------

``:status:``
   One of ``draft``, ``approved``, ``implemented``, ``verified``,
   ``rejected``.
``:component:``
   Affected component(s): ``blip``, ``proto``, ``outpost``.
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
