//! What the detector can name, and the words a person would search for
//! each by.
//!
//! The 80 COCO classes, in the order the model's output columns use. The
//! order is the contract with the model file: change it and every label
//! is wrong, which is why it is tested rather than trusted.
//!
//! Search uses PostgreSQL's `simple` configuration, which does not stem —
//! deliberately, since a library holds documents in many languages. That
//! means "dogs" would not find "dog" on its own, so each class carries
//! the plural and the everyday names people actually type.

/// One thing the detector can recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Class {
    /// The label shown to a person: what the model was trained to call it.
    pub name: &'static str,
    /// Other words that should find it. Never shown as the label.
    pub also: &'static [&'static str],
}

impl Class {
    /// Everything search should match for this class, as one string.
    pub fn search_words(&self) -> String {
        std::iter::once(self.name)
            .chain(self.also.iter().copied())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

const fn class(name: &'static str, also: &'static [&'static str]) -> Class {
    Class { name, also }
}

/// In model output order. Index `i` here is column `5 + i` of a row.
pub const CLASSES: [Class; 80] = [
    class(
        "person",
        &["people", "persons", "human", "man", "woman", "child"],
    ),
    class("bicycle", &["bicycles", "bike", "bikes", "cycle"]),
    class("car", &["cars", "automobile"]),
    class("motorcycle", &["motorcycles", "motorbike", "motorbikes"]),
    class(
        "airplane",
        &["airplanes", "plane", "planes", "aeroplane", "aircraft"],
    ),
    class("bus", &["buses"]),
    class("train", &["trains", "railway"]),
    class("truck", &["trucks", "lorry", "lorries"]),
    class("boat", &["boats", "ship", "ships"]),
    class("traffic light", &["traffic lights"]),
    class("fire hydrant", &["fire hydrants", "hydrant"]),
    class("stop sign", &["stop signs"]),
    class("parking meter", &["parking meters"]),
    class("bench", &["benches"]),
    class("bird", &["birds"]),
    class("cat", &["cats", "kitten", "kittens"]),
    class("dog", &["dogs", "puppy", "puppies"]),
    class("horse", &["horses"]),
    class("sheep", &[]),
    class("cow", &["cows", "cattle"]),
    class("elephant", &["elephants"]),
    class("bear", &["bears"]),
    class("zebra", &["zebras"]),
    class("giraffe", &["giraffes"]),
    class("backpack", &["backpacks", "rucksack"]),
    class("umbrella", &["umbrellas"]),
    class("handbag", &["handbags", "bag", "purse"]),
    class("tie", &["ties", "necktie"]),
    class("suitcase", &["suitcases", "luggage"]),
    class("frisbee", &["frisbees"]),
    class("skis", &["ski", "skiing"]),
    class("snowboard", &["snowboards", "snowboarding"]),
    class("sports ball", &["ball", "balls", "football", "soccer"]),
    class("kite", &["kites"]),
    class("baseball bat", &["baseball bats", "baseball"]),
    class("baseball glove", &["baseball gloves", "baseball"]),
    class("skateboard", &["skateboards", "skateboarding"]),
    class("surfboard", &["surfboards", "surfing"]),
    class("tennis racket", &["tennis rackets", "racket", "tennis"]),
    class("bottle", &["bottles"]),
    class("wine glass", &["wine glasses", "wine"]),
    class("cup", &["cups", "mug", "mugs"]),
    class("fork", &["forks"]),
    class("knife", &["knives"]),
    class("spoon", &["spoons"]),
    class("bowl", &["bowls"]),
    class("banana", &["bananas"]),
    class("apple", &["apples"]),
    class("sandwich", &["sandwiches"]),
    class("orange", &["oranges"]),
    class("broccoli", &[]),
    class("carrot", &["carrots"]),
    class("hot dog", &["hot dogs", "hotdog"]),
    class("pizza", &["pizzas"]),
    class("donut", &["donuts", "doughnut", "doughnuts"]),
    class("cake", &["cakes"]),
    class("chair", &["chairs"]),
    class("couch", &["couches", "sofa", "sofas"]),
    class(
        "potted plant",
        &["potted plants", "plant", "plants", "houseplant"],
    ),
    class("bed", &["beds"]),
    class("dining table", &["dining tables", "table", "tables"]),
    class("toilet", &["toilets"]),
    class("tv", &["television", "televisions", "tvs"]),
    class("laptop", &["laptops", "computer"]),
    class("mouse", &["computer mouse"]),
    class("remote", &["remotes", "remote control"]),
    class("keyboard", &["keyboards"]),
    class(
        "cell phone",
        &["cell phones", "phone", "phones", "mobile", "smartphone"],
    ),
    class("microwave", &["microwaves"]),
    class("oven", &["ovens"]),
    class("toaster", &["toasters"]),
    class("sink", &["sinks"]),
    class("refrigerator", &["refrigerators", "fridge", "fridges"]),
    class("book", &["books"]),
    class("clock", &["clocks"]),
    class("vase", &["vases"]),
    class("scissors", &[]),
    class("teddy bear", &["teddy bears", "teddy", "stuffed animal"]),
    class("hair drier", &["hair dryer", "hairdryer"]),
    class("toothbrush", &["toothbrushes"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_order_matches_the_model_at_the_ends_and_the_middle() {
        // Spot checks against the published COCO ordering. If these move,
        // every label written so far names the wrong thing.
        assert_eq!(CLASSES[0].name, "person");
        assert_eq!(CLASSES[16].name, "dog");
        assert_eq!(CLASSES[39].name, "bottle");
        assert_eq!(CLASSES[62].name, "tv");
        assert_eq!(CLASSES[79].name, "toothbrush");
    }

    #[test]
    fn every_name_is_distinct() {
        let mut names: Vec<_> = CLASSES.iter().map(|class| class.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CLASSES.len());
    }

    #[test]
    fn a_class_is_found_by_its_own_name_and_the_words_people_use() {
        let person = CLASSES[0].search_words();
        assert!(person.starts_with("person "));
        assert!(person.contains("people"));

        let dog = CLASSES[16].search_words();
        assert!(dog.contains("dogs"));
        assert!(dog.contains("puppy"));
    }

    #[test]
    fn a_class_with_no_other_words_is_just_its_name() {
        let sheep = CLASSES.iter().find(|class| class.name == "sheep").unwrap();
        assert_eq!(sheep.search_words(), "sheep");
    }

    #[test]
    fn search_words_are_lowercase_so_they_match_what_is_typed() {
        for class in CLASSES {
            let words = class.search_words();
            assert_eq!(
                words,
                words.to_lowercase(),
                "{} is not lowercase",
                class.name
            );
        }
    }
}
