-- Plan: Implement PeerAuth in pane-proto
--
-- Pre: pane-proto infrastructure exists
-- Post: PeerAuth enum with Kernel/Certificate variants
-- Frame: only pane-proto touched
--
-- Type-checking this module proves the plan is well-formed.

module PeerAuthPlan where

open import Plan
open import Data.Bool using (Bool; true; false)
open import Relation.Binary.PropositionalEquality using (refl)

-- Current project state: all four crate bases exist.
current : State
current proto-infra   = true
current session-infra = true
current app-infra     = true
current fs-infra      = true
current has-peer-auth = false
current has-handshake = false
current has-display   = false
current has-declare   = false
current has-cancel    = false

-- Action: implement PeerAuth
-- Requires pane-proto infrastructure. Produces has-peer-auth.
implement-peer-auth : Action
implement-peer-auth = record
  { pre   = ⟨ proto-infra ⟩
  ; post  = ⟨ has-peer-auth ⟩
  ; label = has-peer-auth ∷ []
  }
  where open import Data.List using (_∷_; [])

-- The plan
plan : Plan
plan = implement-peer-auth ▸ done

-- Goal: current state with PeerAuth added
goal : State
goal = current ⊔ ⟨ has-peer-auth ⟩

-- Proof: plan transforms current to goal.
-- Type-checks iff proto-infra ∈ current (pre satisfied).
proof : Valid current plan goal
proof = valid-step pre-ok valid-done
  where
    pre-ok : ∀ c → c ∈ ⟨ proto-infra ⟩ → c ∈ current
    pre-ok proto-infra refl = refl
