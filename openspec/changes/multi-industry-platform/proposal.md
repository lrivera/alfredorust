# Multi-industry platform (vision note)

> Status: **idea / deferred** — captured 2026-06-18 from a product discussion.
> Not scheduled. No code changes yet. Revisit after the SPA stabilizes.

## Why

The data model was born around a CNC precision-machining shop, but the
primitives are actually generic enough to fit many service businesses. The goal
is for the same logic to "cuadrar en cualquier negocio" (e.g. a salon booking
appointments, a clinic, a consultancy), without forking the product per
industry.

The core abstraction we already have is:

> **Work** (with stages) → split into **units** (with status) → that consume
> **resources** (with hourly cost) → all crossed with **money** (planned entries
> / transactions).

That maps cleanly onto most service businesses; what differs is the *vocabulary*
and one *missing primitive*.

## Example mapping — a salon ("estética")

| Current primitive | CNC shop | Salon |
|---|---|---|
| Project | manufacturing job | a client's visit/service |
| Concept statuses (configurable) | Pedidos→Producción→Entregado | Agendado→En proceso→Terminado |
| Project concept | part ("engrane A") | service ("corte", "tinte") |
| Resource (hourly cost) | machine / operator | stylist / chair / room |
| Resource-usage grid | machine-hours per part | stylist hours per client |
| Contact | client | client |
| Finance | job income | service charge |

The hourly resource-usage grid is already close to a scheduling tool (rows ×
hours × assigned resource); today it's used to allocate *cost*, retrospectively.

## What changes (when taken up)

1. **Configurable vocabulary per company (industry presets).** Let a tenant
   relabel Project/Concept/Resource to e.g. Cita/Servicio/Estilista. Statuses are
   already configurable (`ConceptStatus`). This is mostly copy, not model — the
   cheapest large win in adaptability.
2. **New primitive: Appointment/Booking + calendar (optional module).** What the
   grid does not cover: a time-boxed reservation (start + duration + assigned
   resource + client) with **conflict/availability checking**, and a forward
   **calendar** view. The grid is per-day hourly capture (backward-looking); an
   agenda is prospective and must prevent double-booking. (Customer-facing
   self-booking is a possible later layer.)
3. **Do not multiply entities per industry.** Reuse Project + Resource + grid;
   add only the appointment/calendar module for businesses that need scheduling.

## Orders vs Projects, in this light

They stop looking redundant if framed as **two depths of the same "work"
concept**: an Order is flat/quick work (line items + amount, completes into a
transaction); a Project is staged work with concepts + resource usage. They have
**no DB link today**. Options for later: (a) keep both but make them
configurable/optional per industry, or (b) link them (e.g. an accepted order
generates a production project, or a project rolls up billing orders). Decide
based on real workflows; don't merge prematurely.

## Out of scope / open questions

- Exact appointment data model and conflict rules (per-resource vs per-room).
- Whether "Order" survives as a distinct entity or becomes a configuration of
  "Project".
- Customer-facing booking and notifications (needs the email/SMS service that is
  also still pending — see the username/email note).
