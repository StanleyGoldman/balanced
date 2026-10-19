> **DRAFT: awaiting legal review.** This text has not yet been reviewed by a lawyer.
> It may change before the repository goes public. See
> [ADR 0009](docs/adr/0009-gpl-3-or-later-with-a-plugin-exception-and-dco-sign-off.md).

# Balanced Plugin Exception

SPDX identifier: `AdditionRef-Balanced-Plugin-Exception`

This is an additional permission under section 7 of the GNU General Public License,
version 3 ("GPLv3"). It applies to Balanced, whose license is
`GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception`.

The structure and several definitions follow the GCC Runtime Library Exception,
version 3.1. The idea of a designated extension interface follows the plugin
exceptions used by Qt and KDE projects.

## Preamble

Balanced is free software, and changes to Balanced stay free: anyone who
distributes a modified Balanced must do so under the GPL. This exception adds one
permission on top of that. An extension written to work with Balanced only
through the interfaces Balanced provides for extensions may be distributed under
terms of its author's choosing, including proprietary terms, even when it is
combined with Balanced. The permission is the same for everyone, the copyright
holders of Balanced included.

## 0. Definitions

A file is an "Interface File" if it is part of a crate or module listed in
section 3 as designated.

The "Extension Interface" is the set of all Interface Files, together with the
types, traits, functions, macros and other items they make public.

"Balanced" means the program distributed under this license, including the
Extension Interface, and any modified version of it.

An "Extension" is a work that is not derived from Balanced except through the
Extension Interface. A work is derived from Balanced through the Extension
Interface when it uses the Extension Interface as that interface intends: by
calling it, implementing its traits, using its types, expanding its macros, or
instantiating its generic items, including where doing so copies code from
Interface Files into the Extension's own object code. A work that copies,
modifies or calls into any other part of Balanced is not an Extension.

A "Combined Work" is a work formed by combining one or more Extensions with
Balanced, whether by static or dynamic linking, by loading at run time, or by
distributing them together.

## 1. Grant of additional permission

You have permission to convey an Extension, and a Combined Work, under terms of
your choice for the Extension, even if the Extension's object code includes
material from Interface Files, provided that:

1. Balanced itself, as included in the Combined Work, remains under the GPL with
   this exception, and you meet the GPL's requirements for it, including offering
   its Corresponding Source; and
2. the Extension interacts with Balanced only through the Extension Interface.

## 2. Changes to Balanced stay under the GPL

This exception gives no permission to convey a modified Balanced, or any part of
one, under terms other than the GPL. Code added to Balanced itself, including
code added to Interface Files, is part of Balanced, not an Extension.

## 3. Designated Extension Interface

The copyright holders of Balanced designate the Extension Interface by listing
crates and modules below. They designate only interfaces built for extensions to
use, not internal parts of Balanced. A crate or module removed from the list in a
later version of Balanced remains designated for the versions that listed it.

Designated crates and modules:

- *(none yet)*

## 4. GPL versions

Balanced is licensed under GPLv3 or, at your option, any later version. This
exception applies equally to whichever of those versions you use Balanced under.
Where this text says "the GPL", it means that version.

## 5. Removing this exception

As section 7 of GPLv3 allows, when you convey a copy of Balanced, or of a work
based on it, you may remove this additional permission from that copy. A copy
from which it has been removed is under the GPL alone, and Extensions combined
with that copy receive no permission from this exception.
