use btree_plus_store::{BTreeMap, BTreeStore};

#[test]
fn shared_between_2() {
    // create a shareable slab
    let store = BTreeStore::new();

    // create 2 maps in our shareable slab
    let mut movie_reviews = BTreeMap::new_in(&store);
    let mut book_reviews = BTreeMap::new_in(&store);

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
    if movie_reviews.contains_key(&"Les Misérables") {
        panic!(
            "We've got {} reviews, but Les Misérables ain't one.",
            movie_reviews.len()
        );
    }
    if book_reviews.contains_key(&"Introduction to COBOL") {
        panic!(
            "We've got {} reviews, but Introduction to COBOL ain't one.",
            book_reviews.len()
        );
    }

    // oops, this review has a lot of spelling mistakes, let's delete it.
    movie_reviews
        .remove(&"The Blues Brothers")
        .expect("This review should exist");

    // delete some book reviews
    book_reviews
        .remove(&"Introduction to Java")
        .expect("This review should exist");
    book_reviews
        .remove(&"Percy Jackson and the Lightning Thief")
        .expect("This review should exist");
    let None = book_reviews.remove(&"Percy Jackson and the Lightning Thief") else {
        panic!("This review was already removed")
    };

    // look up the values associated with some keys.
    let to_find = ["Up!", "Office Space", "Hamlet", "Hunger Games"];
    assert_eq!(
        to_find
            .iter()
            .map(|book_or_movie| (
                movie_reviews.get(book_or_movie).map(|e| *e),
                book_reviews.get(book_or_movie).map(|e| *e)
            ))
            .collect::<Vec<_>>(),
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
fn shared_between_3() {
    let store = BTreeStore::new();

    let mut map1 = BTreeMap::new_in(&store);
    let mut map2 = BTreeMap::new_in(&store);
    let mut map3 = BTreeMap::new_in(&store);

    // Insert key-value pairs into map1
    map1.insert(1, "One");
    map1.insert(2, "Two");
    map1.insert(3, "Three");

    // Insert key-value pairs into map2
    map2.insert(1, "Uno");
    map2.insert(2, "Dos");
    map2.insert(3, "Tres");

    // Insert key-value pairs into map3
    map3.insert(1, "Eins");
    map3.insert(2, "Zwei");
    map3.insert(3, "Drei");

    // Verify that the maps have the correct values
    assert_eq!(map1.get(&1).map(|x| *x), Some("One"));
    assert_eq!(map2.get(&1).map(|x| *x), Some("Uno"));
    assert_eq!(map3.get(&1).map(|x| *x), Some("Eins"));

    assert_eq!(map1.get(&2).map(|x| *x), Some("Two"));
    assert_eq!(map2.get(&2).map(|x| *x), Some("Dos"));
    assert_eq!(map3.get(&2).map(|x| *x), Some("Zwei"));

    assert_eq!(map1.get(&3).map(|x| *x), Some("Three"));
    assert_eq!(map2.get(&3).map(|x| *x), Some("Tres"));
    assert_eq!(map3.get(&3).map(|x| *x), Some("Drei"));
}
