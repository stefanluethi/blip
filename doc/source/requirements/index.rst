Requirements
============

Requirements are managed with `sphinx-needs
<https://sphinx-needs.readthedocs.io>`_. Each item has a unique ID and is
linked to the items it refines:

.. graphviz::

   digraph trace {
      rankdir=LR;
      node [shape=box, style="rounded,filled", fontname="sans-serif"];
      edge [fontname="sans-serif", fontsize=10];
      stk  [label="Stakeholder\nSTK_*",   fillcolor="#BFD8D2"];
      req  [label="Requirement\nREQ_*",   fillcolor="#FEDCD2"];
      arch [label="Architecture\nARCH_*", fillcolor="#DCB239"];
      test [label="Test case\nTEST_*",    fillcolor="#DF744A"];
      req  -> stk [label="satisfies"];
      arch -> req [label="satisfies"];
      test -> req [label="verifies"];
   }

.. toctree::
   :maxdepth: 2

   stakeholder
   system
   tests
   traceability
   authoring
