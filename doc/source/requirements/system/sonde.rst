Sonde
=====

The sonde library for targets and unit tests. Its log API is in
:need:`REQ_SONDE_LOG`.

.. req:: Hello of sonde
   :id: REQ_SONDE_HELLO
   :status: implemented
   :component: sonde
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   sonde shall send a ``Hello`` at the start of what it sends, and write it
   first to a recording, with the target name and clock the user gives and
   itself and its version as the sender.

.. req:: Metadata in sonde
   :id: REQ_SONDE_METADATA
   :status: implemented
   :component: sonde
   :satisfies: STK_DEVICE_IF, STK_MEANING

   The API of sonde shall let the user declare, for a trace, its unit, a
   description, labels of integer values and an expected range, and shall send
   them as ``TraceMetadata``.

.. req:: Blocks of samples in sonde
   :id: REQ_SONDE_SAMPLES
   :status: implemented
   :component: sonde
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   The API of sonde shall let the user send many samples of one trace as one
   ``Samples`` message, at a fixed period or at their timestamps.
