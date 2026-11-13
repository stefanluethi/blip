outpost API
===========

The Python package ``blip_outpost``, for scripts that connect a target to blip.
How to use it is in :doc:`/outpost/index`; this page lists all of it. The
descriptions of the functions and classes below are taken from the docstrings
in the code.

Summary
-------

Everything a script needs is exported by the ``blip_outpost`` package:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Name
     - Purpose
   * - ``start``, ``stop``, ``wait``
     - serve blip from a thread of its own, stop, block until :kbd:`Ctrl+C`
   * - ``value``, ``series``, ``unit``, ``metadata``, ``log``, ``send``
     - send to blip, from any thread, each message with its full path
   * - ``batch``
     - hand what a thread sends in a block over at once, with one timestamp
   * - ``get_tracer``, ``Tracer``
     - a tracer by name, for a program that traces itself: paths below its
       name, log records at it, ``debug`` … ``error``
   * - ``LogHandler``
     - sends the records of Python's ``logging`` to blip
   * - ``action``, ``register``
     - make a function an action, add the action methods of an object
   * - ``drain``, ``wait_for_client``
     - wait for room at the blips, wait for a blip
   * - ``get_outpost``
     - the ``Outpost`` the functions above use, e.g. for its ``actions``
   * - ``serve``
     - that outpost configured for ``async with``
   * - ``Outpost``
     - an outpost of its own: the methods above, ``async with``,
       ``drain_async``, ``wait_for_client_async``, ``actions``, ``url``,
       ``clients``, ``name`` and ``clock`` of the ``Hello``, ``send_hello``, and
       ``on_invoke`` to carry out invocations in place of
       the actions, e.g. by forwarding them
   * - ``Actions``
     - the registry of actions: ``add``, ``remove``, ``clear``,
       ``set_enabled``, ``is_enabled``, ``batch``, ``invoke``
   * - ``Range``, ``Unit``
     - ``Annotated`` metadata of action parameters; ``Unit`` and ``Range``
       also of the fields of a dataclass record
   * - ``Description``, ``Labels``
     - ``Annotated`` metadata of the fields of a dataclass record
   * - ``ActionError``
     - raised by an action to reject an invocation
   * - ``LogLevel``
     - ``TRACE``, ``DEBUG``, ``INFO``, ``WARN``, ``ERROR``
   * - ``Message``, ``Data``
     - building messages by hand: ``Message.create_value``,
       ``create_metadata``, ``create_samples``, ``create_hello``,
       ``create_log``, ``create_actions``
   * - ``sleep_until``
     - sleep until a ``time.perf_counter()`` deadline in asyncio code

Module functions
----------------

The functions of the module use one outpost of the script, see `get_outpost`.

.. currentmodule:: blip_outpost

.. autofunction:: start
.. autofunction:: stop
.. autofunction:: wait
.. autofunction:: serve
.. autofunction:: value
.. autofunction:: series
.. autofunction:: unit
.. autofunction:: metadata
.. autofunction:: log
.. autofunction:: send
.. autofunction:: batch
.. autofunction:: get_tracer
.. autofunction:: drain
.. autofunction:: wait_for_client
.. autofunction:: action
.. autofunction:: register
.. autofunction:: get_outpost
.. autofunction:: sleep_until

Outpost
-------

.. autoclass:: Outpost
   :members:

Tracers
-------

.. autoclass:: Tracer
   :members:

.. autoclass:: LogHandler

Actions
-------

.. autoclass:: Actions
   :members:

.. autoclass:: Range
.. autoclass:: Unit
.. autoclass:: Description
.. autoclass:: Labels
.. autoexception:: ActionError

Messages
--------

.. autoclass:: LogLevel
   :members:
   :undoc-members:

.. autoclass:: Message
   :members: create_value, create_metadata, create_samples, create_hello, create_log, create_actions, encode_cbor

.. py:data:: Data

   The type of a value: ``None``, ``int``, ``float``, ``bool``, ``str``, a
   ``list`` of ``Data``, a ``dict`` of ``str`` to ``Data`` or a dataclass whose
   fields are ``Data``; what has a ``tolist`` method, e.g. a numpy array, is
   sent as what it returns.
