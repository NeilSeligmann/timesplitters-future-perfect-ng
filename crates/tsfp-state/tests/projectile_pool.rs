use tsfp_state::projectile_pool::{
    Allocation, CLASS_CAPACITIES, PoolError, ProjectilePool, ProjectileState, SLOT_COUNT,
};

#[test]
fn partitions_match_documented_static_class_queues() {
    let pool = ProjectilePool::new();
    assert_eq!(CLASS_CAPACITIES, [20, 15, 30, 20]);
    assert_eq!(CLASS_CAPACITIES.iter().sum::<usize>(), SLOT_COUNT);
    for (class, cap) in CLASS_CAPACITIES.into_iter().enumerate() {
        assert_eq!(pool.class_order(class as u8).unwrap().len(), cap);
    }
}

#[test]
fn allocation_recycles_tail_and_reports_old_identity_before_reuse() {
    let mut pool = ProjectilePool::new();
    let mut first_cycle = Vec::new();
    for _ in 0..=CLASS_CAPACITIES[0] {
        let allocation = pool.allocate(0).unwrap();
        if first_cycle.len() < CLASS_CAPACITIES[0] {
            let Allocation::Ready(id) = allocation else {
                panic!("fresh slot requested cleanup")
            };
            first_cycle.push(id);
        } else {
            let Allocation::CleanupRequired { retired, ticket } = allocation else {
                panic!("occupied tail should require cleanup")
            };
            let old = retired.id;
            assert!(first_cycle.contains(&old));
            assert_eq!(
                pool.state(old),
                Ok(ProjectileState::Active {
                    class: 0,
                    continuous_renewed: false
                })
            );
            assert_eq!(
                pool.renew_continuous(old),
                Err(PoolError::RecycleInProgress)
            );
            assert_eq!(pool.release(old), Err(PoolError::RecycleInProgress));
            let replacement = pool.finish_recycle(ticket).unwrap();
            assert_eq!(pool.finish_recycle(ticket), Err(PoolError::StaleHandle));
            assert!(matches!(pool.state(old), Err(PoolError::StaleHandle)));
            assert_eq!(
                pool.state(replacement),
                Ok(ProjectileState::Active {
                    class: 0,
                    continuous_renewed: false
                })
            );
        }
    }
}

#[test]
fn generations_reject_stale_handles_and_release_returns_slot_to_tail() {
    let mut pool = ProjectilePool::new();
    let Allocation::Ready(first) = pool.allocate(1).unwrap() else {
        panic!()
    };
    pool.release(first).unwrap();
    assert!(matches!(pool.state(first), Ok(ProjectileState::Free)));
    let order = pool.class_order(1).unwrap();
    assert_eq!(order.last(), Some(&first.slot));
    let Allocation::Ready(second) = pool.allocate(1).unwrap() else {
        panic!()
    };
    assert_eq!(second.slot, first.slot);
    assert_ne!(second.generation, first.generation);
    assert_eq!(pool.renew_continuous(first), Err(PoolError::StaleHandle));
}

#[test]
fn continuous_lease_is_one_tick_and_requires_renewal_each_tick() {
    let mut pool = ProjectilePool::new();
    let Allocation::Ready(id) = pool.allocate(2).unwrap() else {
        panic!()
    };
    assert!(!pool.consume_continuous_tick(id).unwrap());
    pool.renew_continuous(id).unwrap();
    assert!(pool.consume_continuous_tick(id).unwrap());
    assert!(!pool.consume_continuous_tick(id).unwrap());
}

#[test]
fn invalid_class_and_repeated_release_are_reported() {
    let mut pool = ProjectilePool::new();
    assert_eq!(pool.allocate(4), Err(PoolError::InvalidClass));
    let Allocation::Ready(id) = pool.allocate(3).unwrap() else {
        panic!()
    };
    pool.release(id).unwrap();
    assert_eq!(pool.release(id), Err(PoolError::StaleHandle));
}
