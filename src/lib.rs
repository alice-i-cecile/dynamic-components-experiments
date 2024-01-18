//! When working with Bevy, I often find myself wanting to spawn entities within a single function that may not have the same set of components.
//! However, doing so is more challenging than it should be!
//! This code explores a few different ways to do this, and the trade-offs of each.
//!
//! We have a component `A` and a component `B`. We want to spawn an entity with either:
//! 1. Just `A`
//! 2. Just `B`
//! 3. `A` and `B`
//!
//! in a single function based on a passed in `ComponentStrategy` argument.

use bevy::prelude::*;

#[derive(Component)]
struct A;

#[derive(Component)]
struct B;

#[allow(dead_code)]
enum ComponentStrategy {
    A,
    B,
    AAndB,
}

/*
/// This is the obvious way to do it,
/// but simply doesn't work due to mismatched arm types
#[test]
fn impl_bundle_return_type() {
    fn spawn_bundle_naive(strategy: &ComponentStrategy) -> impl Bundle {
        match strategy {
            ComponentStrategy::A => (A,),
            ComponentStrategy::B => (B,),
            ComponentStrategy::AAndB => (A, B),
        }
    }
} */

/*
/// If we follow the compiler error, we can try to use `Box<dyn Bundle>`.
/// However, this fails as `Bundle` is not object safe.
#[test]
#[ignore = "Fails as Bundle is not object safe"]
fn impl_boxed_bundle_return_type() {
    fn spawn_bundle_naive(strategy: &ComponentStrategy) -> Box<dyn Bundle> {
        match strategy {
            ComponentStrategy::A => Box::new((A,)),
            ComponentStrategy::B => Box::new((B,)),
            ComponentStrategy::AAndB => Box::new((A, B)),
        }
    }
}
*/

/// We can brute force this, by operating on the world directly.
///
/// This works, but requires blocking access.
#[test]
fn exclusive_world_access() {
    fn spawn_bundle_exclusive_world_access(
        world: &mut World,
        strategy: ComponentStrategy,
    ) -> Entity {
        let mut entity_world_mut = world.spawn_empty();

        match strategy {
            ComponentStrategy::A => entity_world_mut.insert(A).id(),
            ComponentStrategy::B => entity_world_mut.insert(B).id(),
            ComponentStrategy::AAndB => entity_world_mut.insert(A).insert(B).id(),
        }
    }

    let mut world = World::new();
    spawn_bundle_exclusive_world_access(&mut world, ComponentStrategy::A);
    spawn_bundle_exclusive_world_access(&mut world, ComponentStrategy::B);
    spawn_bundle_exclusive_world_access(&mut world, ComponentStrategy::AAndB);
}

/// We could also pass in a mutable reference to `Commands`.
///
/// This can be used in an ordinary system, but requires plumbing through the `Commands` reference.
/// `Commands` references are also overly powerful: they can be used to modify the world in any way.
#[test]
fn ref_mut_commands() {
    use bevy::ecs::system::RunSystemOnce;

    fn spawn_bundle_ref_mut_commands(
        commands: &mut Commands,
        strategy: ComponentStrategy,
    ) -> Entity {
        let mut entity_commands = commands.spawn_empty();

        match strategy {
            ComponentStrategy::A => entity_commands.insert(A).id(),
            ComponentStrategy::B => entity_commands.insert(B).id(),
            ComponentStrategy::AAndB => entity_commands.insert(A).insert(B).id(),
        }
    }

    let mut world = World::new();

    fn my_system(mut commands: Commands) {
        spawn_bundle_ref_mut_commands(&mut commands, ComponentStrategy::A);
        spawn_bundle_ref_mut_commands(&mut commands, ComponentStrategy::B);
        spawn_bundle_ref_mut_commands(&mut commands, ComponentStrategy::AAndB);
    }

    world.run_system_once(my_system);
}

/// We can restrict the power of the `Commands` reference by using `EntityCommands`.
///
/// This requires a bit more boilerplate, as you must spawn your entity first, but is more restrictive.
/// There are some gnarly liftimes involved, but we get an `EntityCommands` back out,
/// allowing us to chain commands together in the ordinary way.
#[test]
fn ref_mut_entity_commands() {
    use bevy::ecs::system::{EntityCommands, RunSystemOnce};

    fn spawn_bundle_ref_mut_entity_commands<'arg, 'w, 's, 'a>(
        commands: &'arg mut EntityCommands<'w, 's, 'a>,
        strategy: ComponentStrategy,
    ) -> &'arg mut EntityCommands<'w, 's, 'a> {
        match strategy {
            ComponentStrategy::A => commands.insert(A),
            ComponentStrategy::B => commands.insert(B),
            ComponentStrategy::AAndB => commands.insert(A).insert(B),
        }
    }

    let mut world = World::new();

    fn my_system(mut commands: Commands) {
        let mut entity_a = commands.spawn_empty();
        spawn_bundle_ref_mut_entity_commands(&mut entity_a, ComponentStrategy::A);

        let mut entity_b = commands.spawn_empty();
        spawn_bundle_ref_mut_entity_commands(&mut entity_b, ComponentStrategy::B);

        let mut entity_a_and_b = commands.spawn_empty();
        spawn_bundle_ref_mut_entity_commands(&mut entity_a_and_b, ComponentStrategy::AAndB);
    }

    world.run_system_once(my_system);
}

/// We can clean up the `EntityCommands` pattern above,
/// by using an extension method on `EntityCommands`.
#[test]
fn entity_commands_simple_extension() {
    use bevy::ecs::system::{EntityCommands, RunSystemOnce};

    trait EntityCommandsExt {
        fn spawn_bundle_by_strategy(&mut self, strategy: ComponentStrategy) -> &mut Self;
    }

    impl EntityCommandsExt for EntityCommands<'_, '_, '_> {
        fn spawn_bundle_by_strategy(&mut self, strategy: ComponentStrategy) -> &mut Self {
            match strategy {
                ComponentStrategy::A => self.insert(A),
                ComponentStrategy::B => self.insert(B),
                ComponentStrategy::AAndB => self.insert(A).insert(B),
            }
        }
    }

    let mut world = World::new();

    fn my_system(mut commands: Commands) {
        let mut entity_a = commands.spawn_empty();
        entity_a.spawn_bundle_by_strategy(ComponentStrategy::A);

        let mut entity_b = commands.spawn_empty();
        entity_b.spawn_bundle_by_strategy(ComponentStrategy::B);

        let mut entity_a_and_b = commands.spawn_empty();
        entity_a_and_b.spawn_bundle_by_strategy(ComponentStrategy::AAndB);
    }

    world.run_system_once(my_system);
}

/// However, this isn't very flexible: now we need a different extension method for every place we want to use this pattern!
///
/// Instead, let's pass in a closure into our extension method,
/// which controls which builder we're using.
///
/// Elaborate setup, but very flexible and quite comforable to use.
#[test]
fn entity_commands_closure_extension() {
    use bevy::ecs::system::{EntityCommands, RunSystemOnce};

    trait EntityCommandsExt<Config> {
        fn spawn_dynamic_bundle(
            &mut self,
            config: Config,
            f: impl FnOnce(Config, &mut Self),
        ) -> &mut Self;
    }

    impl<Config> EntityCommandsExt<Config> for EntityCommands<'_, '_, '_> {
        fn spawn_dynamic_bundle(
            &mut self,
            config: Config,
            f: impl FnOnce(Config, &mut Self),
        ) -> &mut Self {
            f(config, self);
            self
        }
    }

    fn my_dynamic_builder(strategy: ComponentStrategy, commands: &mut EntityCommands<'_, '_, '_>) {
        match strategy {
            ComponentStrategy::A => commands.insert(A),
            ComponentStrategy::B => commands.insert(B),
            ComponentStrategy::AAndB => commands.insert(A).insert(B),
        };
    }

    let mut world = World::new();

    fn my_system(mut commands: Commands) {
        let mut entity_a = commands.spawn_empty();
        entity_a.spawn_dynamic_bundle(ComponentStrategy::A, my_dynamic_builder);

        let mut entity_b = commands.spawn_empty();
        entity_b.spawn_dynamic_bundle(ComponentStrategy::B, my_dynamic_builder);

        let mut entity_a_and_b = commands.spawn_empty();
        entity_a_and_b.spawn_dynamic_bundle(ComponentStrategy::AAndB, my_dynamic_builder);
    }

    world.run_system_once(my_system);
}
