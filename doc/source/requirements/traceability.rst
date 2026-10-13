Traceability
============

All requirements
----------------

.. needtable::
   :types: req
   :columns: id, title, status, component, satisfies, satisfies_back as "Satisfied by", verifies_back as "Verified by"
   :style: table
   :sort: id

Requirements without test
-------------------------

.. needtable::
   :types: req
   :filter: len(verifies_back) == 0 and status != "rejected"
   :columns: id, title, status, component
   :style: table

Requirements not linked to a stakeholder need
---------------------------------------------

.. needtable::
   :types: req
   :filter: len(satisfies) == 0
   :columns: id, title, status
   :style: table

Status overview
---------------

.. needpie:: Requirements by status
   :labels: draft, approved, implemented, verified
   :style: transparent

   type == "req" and status == "draft"
   type == "req" and status == "approved"
   type == "req" and status == "implemented"
   type == "req" and status == "verified"

Trace graph
-----------

.. needflow::
   :types: stk, req, test
   :show_link_names:
