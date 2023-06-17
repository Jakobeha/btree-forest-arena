#![cfg(feature = "concurrent-shareable-slab")]

use std::thread;
use btree_store::concurrent_shareable_slab::{BTreeMap, ShareableSlab};

#[test]
fn shared_between_2() {
    // create a shareable slab
    let slab = ShareableSlab::new();

    // create 2 maps in our shareable slab
    let mut movie_reviews = BTreeMap::new_in(&slab);
    let mut book_reviews = BTreeMap::new_in(&slab);

    // review some movies and books.
    movie_reviews.insert("Office Space", "Deals with real issues in the workplace.");
    book_reviews.insert("Hamlet", "Great book");
    movie_reviews.insert("Pulp Fiction", "Masterpiece.");
    movie_reviews.insert("The Godfather", "Very enjoyable.");
    book_reviews.insert("Percy Jackson and the Lightning Thief", "Great book");
    movie_reviews.insert("Hunger Games", "Better than the book");
    book_reviews.insert("Hunger Games", "Better than the movie");
    book_reviews.insert("Introduction to Rust", "Best");
    book_reviews.insert("Introduction to Java", "Better");
    book_reviews.insert("Introduction to Python", "Good");
    movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");

    // check for a specific one.
    if movie_reviews.contains_key("Les Misérables") {
        panic!("We've got {} reviews, but Les Misérables ain't one.",
               movie_reviews.len());
    }
    if book_reviews.contains_key("Introduction to COBOL") {
        panic!("We've got {} reviews, but Introduction to COBOL ain't one.",
               book_reviews.len());
    }

    // oops, this review has a lot of spelling mistakes, let's delete it.
    movie_reviews.remove("The Blues Brothers").expect("This review should exist");

    // delete some book reviews
    book_reviews.remove("Introduction to Java").expect("This review should exist");
    book_reviews.remove("Percy Jackson and the Lightning Thief").expect("This review should exist");
    let None = book_reviews.remove("Percy Jackson and the Lightning Thief") else {
        panic!("This review was already removed")
    };

    // look up the values associated with some keys.
    let to_find = ["Up!", "Office Space", "Hamlet", "Hunger Games"];
    assert_eq!(
        to_find.iter().map(|book_or_movie| (
            movie_reviews.get(book_or_movie).map(|e| *e),
            book_reviews.get(book_or_movie).map(|e| *e)
        )).collect::<Vec<_>>(),
        [
            (None, None),
            (Some("Deals with real issues in the workplace."), None),
            (None, Some("Great book")),
            (Some("Better than the book"), Some("Better than the movie"))
        ]
    );

    // iterate over and consume everything.
    assert_eq!(
        movie_reviews.into_iter().collect::<Vec<_>>(),
        [
            ("Hunger Games", "Better than the book"),
            ("Office Space", "Deals with real issues in the workplace."),
            ("Pulp Fiction", "Masterpiece."),
            ("The Godfather", "Very enjoyable.")
        ]
    );
    assert_eq!(
        book_reviews.into_iter().collect::<Vec<_>>(),
        [
            ("Hamlet", "Great book"),
            ("Hunger Games", "Better than the movie"),
            ("Introduction to Python", "Good"),
            ("Introduction to Rust", "Best")
        ]
    );
}

#[test]
fn shared_between_2_on_separate_threads() {
    // create a shareable slab
    let slab = ShareableSlab::new();

    thread::scope(|scope| {
        let movie_thread = scope.spawn(|| {
            // create a map in our shareable slab
            let mut movie_reviews = BTreeMap::new_in(&slab);

            // review some movies and books.
            movie_reviews.insert("Office Space", "Deals with real issues in the workplace.");
            movie_reviews.insert("Pulp Fiction", "Masterpiece.");
            movie_reviews.insert("The Godfather", "Very enjoyable.");
            movie_reviews.insert("Hunger Games", "Better than the book");
            movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");

            // check for a specific one.
            if movie_reviews.contains_key("Les Misérables") {
                panic!("We've got {} reviews, but Les Misérables ain't one.",
                       movie_reviews.len());
            }

            // oops, this review has a lot of spelling mistakes, let's delete it.
            movie_reviews.remove("The Blues Brothers").expect("This review should exist");

            // look up the values associated with some keys.
            let to_find = ["Up!", "Office Space", "Hamlet", "Hunger Games"];
            assert_eq!(
                to_find.iter().map(|book_or_movie| {
                    movie_reviews.get(book_or_movie).map(|e| *e)
                }).collect::<Vec<_>>(),
                [None, Some("Deals with real issues in the workplace."), None, Some("Better than the book")]
            );

            // iterate over and consume everything.
            assert_eq!(
                movie_reviews.into_iter().collect::<Vec<_>>(),
                [
                    ("Hunger Games", "Better than the book"),
                    ("Office Space", "Deals with real issues in the workplace."),
                    ("Pulp Fiction", "Masterpiece."),
                    ("The Godfather", "Very enjoyable.")
                ]
            );
        });

        let book_thread = scope.spawn(|| {
            // create another map in our shareable slab
            let mut book_reviews = BTreeMap::new_in(&slab);

            // review some books.
            book_reviews.insert("Hamlet", "Great book");
            book_reviews.insert("Percy Jackson and the Lightning Thief", "Great book");
            book_reviews.insert("Hunger Games", "Better than the movie");
            book_reviews.insert("Introduction to Rust", "Best");
            book_reviews.insert("Introduction to Java", "Better");
            book_reviews.insert("Introduction to Python", "Good");

            // check for a specific one.
            if book_reviews.contains_key("Introduction to COBOL") {
                panic!("We've got {} reviews, but Introduction to COBOL ain't one.",
                       book_reviews.len());
            }

            // delete some book reviews
            book_reviews.remove("Introduction to Java").expect("This review should exist");
            book_reviews.remove("Percy Jackson and the Lightning Thief").expect("This review should exist");
            let None = book_reviews.remove("Percy Jackson and the Lightning Thief") else {
                panic!("This review was already removed")
            };

            // look up the values associated with some keys.
            let to_find = ["Up!", "Office Space", "Hamlet", "Hunger Games"];
            assert_eq!(
                to_find.iter().map(|book_or_movie| {
                    book_reviews.get(book_or_movie).map(|e| *e)
                }).collect::<Vec<_>>(),
                [None, None, Some("Great book"), Some("Better than the movie")]
            );

            assert_eq!(
                book_reviews.into_iter().collect::<Vec<_>>(),
                [
                    ("Hamlet", "Great book"),
                    ("Hunger Games", "Better than the movie"),
                    ("Introduction to Python", "Good"),
                    ("Introduction to Rust", "Best")
                ]
            );
        });

        movie_thread.join().expect("movie thread crashed");
        book_thread.join().expect("book thread crashed");
    });
}
