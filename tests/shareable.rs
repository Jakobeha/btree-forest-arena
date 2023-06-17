#![cfg(feature = "shareable-slab")]

use btree_slab::shareable_slab::ShareableSlab;
use btree_slab::SharingBTreeMap;

#[test]
fn not_shared() {
    // create a shareable slab
    let slab = ShareableSlab::new();

    // create a map in our shareable slab
    let mut movie_reviews = SharingBTreeMap::new_in(&slab);

    // review some movies.
    movie_reviews.insert("Office Space", "Deals with real issues in the workplace.");
    movie_reviews.insert("Pulp Fiction", "Masterpiece.");
    movie_reviews.insert("The Godfather", "Very enjoyable.");
    movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");

    // check for a specific one.
    if movie_reviews.contains_key("Les Misérables") {
        panic!("We've got {} reviews, but Les Misérables ain't one.",
               movie_reviews.len());
    }

    // oops, this review has a lot of spelling mistakes, let's delete it.
    movie_reviews.remove("The Blues Brothers");

    // look up the values associated with some keys.
    let to_find = ["Up!", "Office Space"];
    for movie in &to_find {
        match movie_reviews.get(movie) {
            Some(review) => println!("{}: {}", movie, review),
            None => println!("{} is unreviewed.", movie)
        }
    }

    // Look up the value for a key (will panic if the key is not found).
    // Can't do that because this is a [std::cell::Ref]!
    // println!("Movie review: {}", movie_reviews["Office Space"]);

    // iterate over everything.
    for elem in &movie_reviews {
        let (movie, review) = elem.as_pair();
        println!("{}: \"{}\"", movie, review);
    }
}

#[test]
fn shared_between_2() {
    // create a shareable slab
    let slab = ShareableSlab::new();

    // create 2 maps in our shareable slab
    let mut movie_reviews = SharingBTreeMap::new_in(&slab);
    let mut book_reviews = SharingBTreeMap::new_in(&slab);

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

    // Look up the value for a key (will panic if the key is not found).
    // Can't do that because these are [std::cell::Ref]s!
    // println!("Movie review: {}", movie_reviews["Office Space"]);
    // println!("Book review: {}", book_reviews["Introduction to Rust"]);

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
