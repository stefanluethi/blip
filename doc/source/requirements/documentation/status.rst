Where it stands
===============

.. needtable::
   :types: req
   :filter: component == "doc"
   :columns: id, title, status, priority
   :style: table
   :sort: id

Where each is met:

* :need:`REQ_DOC_KINDS` -- the tutorial is the :doc:`/start/quickstart`, the
  how-to guides are the pages of blip, the outpost and sonde, the reference is
  :doc:`/reference/index`, the explanation is :doc:`/overview/index`,
  :doc:`/overview/concepts` and the internals page of each part.
* :need:`REQ_DOC_OVERVIEW` -- :doc:`/overview/index`.
* :need:`REQ_DOC_PARTS` -- a section for each of :doc:`/blip/index`,
  :doc:`/outpost/index` and :doc:`/sonde/index`, each with its internals.
* :need:`REQ_DOC_ONE_PLACE` -- the user guide and the software architecture,
  which told the concepts and the parts twice, are merged.
* :need:`REQ_DOC_TROUBLESHOOT` -- :doc:`/help/troubleshooting`.
* :need:`REQ_DOC_NEXT` -- *Before you begin* and *Next steps* on the guides.
* :need:`REQ_DOC_CLI_REF` -- :doc:`/reference/outpost-cli`, checked by
  :need:`TEST_DOC_CLI`.
* :need:`REQ_DOC_API_REF` -- :doc:`/reference/outpost-api` from the
  docstrings, :doc:`/reference/sonde-api` from the headers.
* :need:`REQ_DOC_FORMATS` -- :doc:`/reference/files` and
  :doc:`/reference/protocol`.
* :need:`REQ_DOC_CHANGES` -- :doc:`/changes`.
* :need:`REQ_DOC_LIMITS` -- :doc:`/help/limits`.
* :need:`REQ_DOC_FIND` -- ``redirects`` in ``conf.py``, for pages that move.
* :need:`REQ_DOC_LINKS` -- ``make linkcheck``.
* :need:`REQ_DOC_FEEDBACK` -- *Edit this page* and *Report a problem* beside
  each page.

Still open:

* :need:`REQ_DOC_EXAMPLES` -- the Python examples of the outpost guide are
  compiled and the ``minimal`` example it includes is run by
  :need:`TEST_DOC_EXAMPLES`, but the C++ snippets of the sonde guide are not
  compiled.

:doc:`/help/limits` gives no measured message rate of blip itself, as it depends
on the machine and the views; the ``fast`` example measures it.
