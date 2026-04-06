-- Plan verification framework for pane.
-- Plans are sequences of actions with pre/post-conditions.
-- Plan correctness = type-checking.
--
-- Based on Hill, Komendantskaya, Petrick, "Proof-Carrying Plans"
-- (PPDP 2020). Adapted to pane's project state model.
--
-- Capabilities are kept minimal — add new ones as plans need
-- them, not speculatively.

module Plan where

open import Data.Bool using (Bool; true; false; _∨_)
open import Data.List using (List; []; _∷_)
open import Relation.Binary.PropositionalEquality using (_≡_; refl)

-- Capabilities relevant to current plans. Extend as needed.
data Capability : Set where
  proto-infra    : Capability  -- pane-proto base exists (ServiceId, Protocol, Message, etc.)
  session-infra  : Capability  -- pane-session base exists (Transport, Bridge, FrameCodec)
  app-infra      : Capability  -- pane-app base exists (Dispatch, LooperCore, PaneBuilder)
  fs-infra       : Capability  -- pane-fs base exists (AttrReader, AttrSet, PaneEntry)
  has-peer-auth  : Capability  -- PeerAuth enum implemented
  has-handshake  : Capability  -- Hello/Welcome types implemented
  has-display    : Capability  -- Display protocol implemented
  has-declare    : Capability  -- DeclareInterest messages implemented
  has-cancel     : Capability  -- Cancel message implemented

-- Decidable equality, all cases explicit.
_≟_ : Capability → Capability → Bool
proto-infra   ≟ proto-infra   = true
session-infra ≟ session-infra = true
app-infra     ≟ app-infra     = true
fs-infra      ≟ fs-infra      = true
has-peer-auth ≟ has-peer-auth = true
has-handshake ≟ has-handshake = true
has-display   ≟ has-display   = true
has-declare   ≟ has-declare   = true
has-cancel    ≟ has-cancel    = true
proto-infra   ≟ session-infra = false
proto-infra   ≟ app-infra     = false
proto-infra   ≟ fs-infra      = false
proto-infra   ≟ has-peer-auth = false
proto-infra   ≟ has-handshake = false
proto-infra   ≟ has-display   = false
proto-infra   ≟ has-declare   = false
proto-infra   ≟ has-cancel    = false
session-infra ≟ proto-infra   = false
session-infra ≟ app-infra     = false
session-infra ≟ fs-infra      = false
session-infra ≟ has-peer-auth = false
session-infra ≟ has-handshake = false
session-infra ≟ has-display   = false
session-infra ≟ has-declare   = false
session-infra ≟ has-cancel    = false
app-infra     ≟ proto-infra   = false
app-infra     ≟ session-infra = false
app-infra     ≟ fs-infra      = false
app-infra     ≟ has-peer-auth = false
app-infra     ≟ has-handshake = false
app-infra     ≟ has-display   = false
app-infra     ≟ has-declare   = false
app-infra     ≟ has-cancel    = false
fs-infra      ≟ proto-infra   = false
fs-infra      ≟ session-infra = false
fs-infra      ≟ app-infra     = false
fs-infra      ≟ has-peer-auth = false
fs-infra      ≟ has-handshake = false
fs-infra      ≟ has-display   = false
fs-infra      ≟ has-declare   = false
fs-infra      ≟ has-cancel    = false
has-peer-auth ≟ proto-infra   = false
has-peer-auth ≟ session-infra = false
has-peer-auth ≟ app-infra     = false
has-peer-auth ≟ fs-infra      = false
has-peer-auth ≟ has-handshake = false
has-peer-auth ≟ has-display   = false
has-peer-auth ≟ has-declare   = false
has-peer-auth ≟ has-cancel    = false
has-handshake ≟ proto-infra   = false
has-handshake ≟ session-infra = false
has-handshake ≟ app-infra     = false
has-handshake ≟ fs-infra      = false
has-handshake ≟ has-peer-auth = false
has-handshake ≟ has-display   = false
has-handshake ≟ has-declare   = false
has-handshake ≟ has-cancel    = false
has-display   ≟ proto-infra   = false
has-display   ≟ session-infra = false
has-display   ≟ app-infra     = false
has-display   ≟ fs-infra      = false
has-display   ≟ has-peer-auth = false
has-display   ≟ has-handshake = false
has-display   ≟ has-declare   = false
has-display   ≟ has-cancel    = false
has-declare   ≟ proto-infra   = false
has-declare   ≟ session-infra = false
has-declare   ≟ app-infra     = false
has-declare   ≟ fs-infra      = false
has-declare   ≟ has-peer-auth = false
has-declare   ≟ has-handshake = false
has-declare   ≟ has-display   = false
has-declare   ≟ has-cancel    = false
has-cancel    ≟ proto-infra   = false
has-cancel    ≟ session-infra = false
has-cancel    ≟ app-infra     = false
has-cancel    ≟ fs-infra      = false
has-cancel    ≟ has-peer-auth = false
has-cancel    ≟ has-handshake = false
has-cancel    ≟ has-display   = false
has-cancel    ≟ has-declare   = false

-- State: which capabilities are present.
State : Set
State = Capability → Bool

-- Membership
_∈_ : Capability → State → Set
c ∈ s = s c ≡ true

-- Singleton state
⟨_⟩ : Capability → State
⟨ c ⟩ c' = c ≟ c'

-- Union
_⊔_ : State → State → State
(s₁ ⊔ s₂) c = s₁ c ∨ s₂ c

-- Action: pre-condition, post-condition, label
record Action : Set where
  field
    pre  : State
    post : State
    label : List Capability

-- Plan: sequence of actions
data Plan : Set where
  done : Plan
  _▸_  : Action → Plan → Plan

-- Validity: pre-conditions satisfied at each step
data Valid : State → Plan → State → Set where
  valid-done : ∀ {s} → Valid s done s
  valid-step : ∀ {s₀ a p s₂}
    → (∀ c → c ∈ Action.pre a → c ∈ s₀)
    → Valid (s₀ ⊔ Action.post a) p s₂
    → Valid s₀ (a ▸ p) s₂
