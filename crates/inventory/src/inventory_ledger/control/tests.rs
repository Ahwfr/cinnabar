mod authoritative;
mod current_window;
mod replacements;
mod reused_ids;

use protocol::{ContainerCloseEvent, ContainerIdentity, ContainerOpenEvent, InventoryEvent};

use super::super::INVENTORY_REQUEST_TIMEOUT_MILLIS;
use crate::inventory_ledger::{
    GENERIC_STORAGE_WINDOW_TYPE, PlayerInventoryLedger,
    lifecycle_tests::{ledger_with_slot_zero, personal_open},
};

/// Creates a server storage menu, including a replacement that reuses a just-closed ID.
fn storage_open(window_id: i32) -> InventoryEvent {
    InventoryEvent::Open(ContainerOpenEvent {
        container: ContainerIdentity::window(window_id),
        window_type: GENERIC_STORAGE_WINDOW_TYPE,
        position: [0, 64, 0],
        runtime_entity_id: -1,
    })
}

/// Delivers an acknowledgement for a close already admitted to transport.
fn acknowledge_close(ledger: &mut PlayerInventoryLedger, window_id: i32, window_type: i8) {
    ledger.apply(&InventoryEvent::Close(ContainerCloseEvent {
        container: ContainerIdentity::window(window_id),
        window_type,
        server_initiated: false,
    }));
}

#[test]
fn a_cancelled_background_open_closes_when_the_server_answers() {
    let mut ledger = ledger_with_slot_zero();
    assert!(ledger.request_background_personal_open(42));
    assert!(ledger.mark_transport_enqueued(0));
    ledger.request_background_personal_close();
    ledger.apply(&InventoryEvent::Open(personal_open(7)));
    assert!(!ledger.personal_inventory_visible());
    assert!(!ledger.background_personal_active());
    assert!(ledger.mark_transport_enqueued(1));
    acknowledge_close(&mut ledger, 7, super::PERSONAL_INVENTORY_WINDOW_TYPE);
    assert!(!ledger.background_personal_busy());
    assert!(ledger.request_personal_open(42));
    assert!(ledger.personal_inventory_visible());
}

#[test]
fn a_background_window_can_move_a_carried_stack_without_showing_inventory() {
    use crate::inventory_ledger::InventoryTarget;
    use protocol::{InventorySlotEvent, NetworkItemStack, SlotIdentity};
    let mut ledger = ledger_with_slot_zero();
    let stack = ledger.backing_stack(0).unwrap().clone();
    for (slot, stack) in [(0, NetworkItemStack::empty()), (10, stack)] {
        ledger.apply(&InventoryEvent::Slot(InventorySlotEvent {
            identity: SlotIdentity {
                container: ContainerIdentity::window(0),
                slot,
            },
            stack,
            storage_item: None,
        }));
    }
    assert!(ledger.request_background_personal_open(42));
    assert!(!ledger.background_personal_ready());
    assert!(ledger.mark_transport_enqueued(0));
    ledger.apply(&InventoryEvent::Open(personal_open(7)));
    assert!(ledger.background_personal_ready());
    assert!(!ledger.personal_inventory_visible());
    ledger
        .begin_hotbar_swap(InventoryTarget::Player(10), 2)
        .unwrap();
    assert_eq!(ledger.displayed_stack(2).unwrap().count, 32);
    assert!(ledger.displayed_stack(10).is_none());
    assert_eq!(ledger.backing_stack(10).unwrap().count, 32);
}

#[test]
fn mounted_reply_finishes_the_request_without_a_personal_reset_close() {
    let mut ledger = ledger_with_slot_zero();
    assert!(ledger.request_personal_open(900));
    assert!(ledger.mark_transport_enqueued(0));
    ledger.apply(&InventoryEvent::Open(ContainerOpenEvent {
        container: ContainerIdentity::window(7),
        window_type: protocol::WINDOW_TYPE_HORSE,
        position: [0; 3],
        runtime_entity_id: -91,
    }));
    assert_eq!(ledger.window_actor(), Some(-91));
    ledger.poll_timeout(INVENTORY_REQUEST_TIMEOUT_MILLIS);
    assert!(ledger.pending_batch().unwrap().is_none());
    ledger.request_storage_close();
    assert_eq!(
        ledger.pending_batch().unwrap().unwrap().0,
        protocol::container_close_packet(7).unwrap()
    );
    assert!(ledger.mark_transport_enqueued(INVENTORY_REQUEST_TIMEOUT_MILLIS + 1));
    acknowledge_close(&mut ledger, 7, protocol::WINDOW_TYPE_HORSE);
    assert!(ledger.request_personal_open(901));
    assert_eq!(
        ledger.pending_batch().unwrap().unwrap().0,
        protocol::open_inventory_packet(901).unwrap()
    );
}
