extern crate time;
extern crate csv;
extern crate rand;
// #[macro_use]
// extern crate serde_derive;
// #[macro_use]
// extern crate rocket;

use std::path::Path;
use std::env;
use std::thread;
use time::PreciseTime;
use std::fs::OpenOptions;
use std::error::Error;
use std::iter::FromIterator;
use rand::distributions::{IndependentSample, Range};
use std::collections::HashMap;
use std::collections::HashSet;
// use rocket::http::RawStr;

static FULL_DECK: Deck = Deck {
    cards: [32, 32, 32, 32, 32, 32, 32, 32, 32, 128],
    size: 416,
};

#[derive(Debug,PartialOrd,PartialEq,Clone,Copy,Hash,Eq)]
enum Score {
    Bust,
    Value(u16),
    SixCardCharlie(u16),
    Natural,
}

#[test]
fn two_naturals_equal() {
    assert!(Score::Natural == Score::Natural);
}

#[test]
fn greater_five_card_charlie_wins() {
    assert!(Score::SixCardCharlie(18) > Score::SixCardCharlie(17));
}

#[test]
fn greater_value_wins() {
    assert!(Score::Value(18) > Score::Value(17));
}

#[test]
fn value_beats_bust() {
    assert!(Score::Value(8) > Score::Bust);
}

#[test]
fn five_card_charlie_beats_value() {
    assert!(Score::SixCardCharlie(12) > Score::Value(15));
}

#[test]
fn natural_beats_five_card_charlie() {
    assert!(Score::Natural > Score::SixCardCharlie(21));
}

#[test]
fn five_card_charlie_loses_to_natural() {
    assert!(Score::SixCardCharlie(21) < Score::Natural);
}

#[test]
fn equal_values_are_equal() {
    assert!(Score::Value(16) == Score::Value(16));
}

fn hand_value(hand: &Vec<u16>) -> u16 {
    let mut has_ace = false;
    let mut score = 0;
    for &card in hand.iter() {
        score += card;
        if card == 1 {
            has_ace = true;
        }
    }
    if score < 12 && has_ace {
        return score + 10;
    }
    score
}

#[test]
fn hard_hand_values() {
    assert!(20 == hand_value(&vec![10, 10]));
    assert!(21 == hand_value(&vec![10, 10, 1]));
    assert!(30 == hand_value(&vec![10, 10, 10]));
    assert!(12 == hand_value(&vec![10, 1, 1]));
    assert!(0 == hand_value(&vec![]));
}

#[test]
fn soft_hand_values() {
    assert!(11 == hand_value(&vec![1]));
    assert!(12 == hand_value(&vec![1, 1]));
    assert!(21 == hand_value(&vec![10, 1]))
}

fn min_hand_value(hand: &Vec<u16>) -> u16 {
    hand.iter().sum()
}

#[test]
fn min_hand_values() {
    assert!(1 == min_hand_value(&vec![1]));
    assert!(0 == min_hand_value(&vec![]));
    assert!(15 == min_hand_value(&vec![10, 5]));
    assert!(11 == min_hand_value(&vec![10, 1]));
}

fn score(hand: &Vec<u16>) -> Score {
    let hand_total = hand_value(hand);
    if hand_total > 21 {
        return Score::Bust;
    } else if hand.len() == 6 {
        return Score::SixCardCharlie(hand_total);
    } else if hand.len() == 2 && hand_total == 21 {
        return Score::Natural;
    }
    Score::Value(hand_total)
}

#[test]
fn test_score() {
    assert!(Score::Natural == score(&vec![1, 10]));
    assert!(Score::Natural == score(&vec![10, 1]));
    assert!(Score::SixCardCharlie(20) == score(&vec![2, 2, 2, 2, 1, 1]));
    assert!(Score::SixCardCharlie(19) == score(&vec![2, 3, 3, 3, 3, 5]));
    assert!(Score::Bust == score(&vec![10, 10, 2]));
    assert!(Score::Value(21) == score(&vec![10, 10, 1]));
    assert!(Score::Value(16) == score(&vec![4, 4, 4, 4]));
}

fn hand_expectation(player_score: Score, dealer_score: Score) -> f32 {
    match player_score {
        Score::Bust => -1.0,
        Score::Natural => {
            if dealer_score == Score::Natural {
                0.0
            } else {
                1.5
            }
        }
        _ => {
            if player_score > dealer_score {
                1.0
            } else if dealer_score > player_score {
                -1.0
            } else {
                0.0
            }
        }
    }
}

#[test]
fn test_hand_expectation() {
    assert!(-1.0 == hand_expectation(Score::Bust, Score::Bust));
    assert!(-1.0 == hand_expectation(Score::Bust, Score::Natural));
    assert!(-1.0 == hand_expectation(Score::Value(10), Score::Value(11)));
    assert!(0.0 == hand_expectation(Score::Value(10), Score::Value(10)));
    assert!(1.0 == hand_expectation(Score::Value(11), Score::Value(10)));
    assert!(-1.0 == hand_expectation(Score::Value(21), Score::SixCardCharlie(16)));
    assert!(-1.0 == hand_expectation(Score::SixCardCharlie(10), Score::SixCardCharlie(11)));
    assert!(0.0 == hand_expectation(Score::SixCardCharlie(10), Score::SixCardCharlie(10)));
    assert!(1.0 == hand_expectation(Score::SixCardCharlie(11), Score::SixCardCharlie(10)));
    assert!(1.5 == hand_expectation(Score::Natural, Score::SixCardCharlie(21)));
    assert!(0.0 == hand_expectation(Score::Natural, Score::Natural));
}

fn card_index(card: u16) -> usize {
    card as usize - 1
}

// fn card_at(index: usize) -> u16 {
//     index as u16 + 1
// }

#[derive(Debug)]
struct GameState {
    player: Vec<u16>,
    dealer: Vec<u16>,
    deck: Deck,
    failed_insurance: bool,
    is_split: bool,
    first_split_hand: bool,
}

#[derive(Eq,PartialEq,Hash,Debug,Clone,Copy)]
struct Deck {
    cards: [u16; 10],
    size: u16,
}

impl Deck {
    fn draw(&mut self, card: u16) {
        assert!(self.cards[card_index(card)] > 0);
        self.cards[card_index(card)] -= 1;
        self.size -= 1;
    }
    fn replace(&mut self, card: u16) {
        self.cards[card_index(card)] += 1;
        self.size += 1;
    }
    fn draw_to(&mut self, hand: &mut Vec<u16>, card: u16) {
        self.draw(card);
        hand.push(card);
    }
    fn replace_from(&mut self, hand: &mut Vec<u16>, card: u16) {
        match hand.iter().position(|&c| c == card) {
            Some(index) => {
                hand.swap_remove(index);
                self.replace(card);
            }
            None => panic!("Replacing a card not in the player's hand"),
        }
    }
    fn card_prob(&self, card: u16, cant_be_ten: bool) -> f32 {
        if cant_be_ten && card == 10 {
            0.0
        } else if cant_be_ten {
            self.cards[card_index(card)] as f32 / (self.size - self.cards[card_index(10)]) as f32
        } else {
            self.cards[card_index(card)] as f32 / self.size as f32
        }
    }
}

#[test]
fn test_valid_draw() {
    let mut deck = Deck {
        cards: [1, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 49,
    };
    deck.draw(1);
    assert!(0 == deck.cards[0]);
    assert!(48 == deck.size);
}

#[test]
#[should_panic]
fn test_invalid_draw() {
    let mut deck = Deck {
        cards: [0, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 48,
    };
    deck.draw(1);
}

#[test]
fn test_replace() {
    let mut deck = Deck {
        cards: [0, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 48,
    };
    deck.replace(1);
    assert!(1 == deck.cards[0]);
    assert!(49 == deck.size);
}

#[test]
fn test_valid_draw_to() {
    let mut hand = vec![];
    let mut deck = Deck {
        cards: [1, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 49,
    };
    deck.draw_to(&mut hand, 1);
    assert!(0 == deck.cards[0]);
    assert!(48 == deck.size);
    assert!(vec![1] == hand);
}

#[test]
#[should_panic]
fn test_invalid_draw_to() {
    let mut hand = vec![];
    let mut deck = Deck {
        cards: [0, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 48,
    };
    deck.draw_to(&mut hand, 1);
}

#[test]
fn test_valid_replace_from() {
    let mut hand = vec![1, 1];
    let mut deck = Deck {
        cards: [0, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 48,
    };
    deck.replace_from(&mut hand, 1);
    assert!(1 == deck.cards[0]);
    assert!(49 == deck.size);
    assert!(vec![1] == hand);
    deck.replace_from(&mut hand, 1);
    assert!(2 == deck.cards[0]);
    assert!(50 == deck.size);
    assert!(Vec::<u16>::new() == hand);
}

#[test]
#[should_panic]
fn test_invalid_replace_from() {
    let mut hand = vec![];
    let mut deck = Deck {
        cards: [0, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 48,
    };
    deck.replace_from(&mut hand, 1);
}

#[test]
fn test_card_prob() {
    let deck = Deck {
        cards: [0, 4, 4, 4, 4, 4, 4, 4, 4, 16],
        size: 48,
    };
    assert!(0.0 == deck.card_prob(1, false));
    assert!(4.0 / 48.0 == deck.card_prob(2, false));
    assert!(4.0 / 32.0 == deck.card_prob(2, true));
    assert!(0.0 == deck.card_prob(10, true));
    assert!(16.0 / 48.0 == deck.card_prob(10, false));
}

fn dealer_stands(hand: &Vec<u16>) -> bool {
    hand.len() == 6 || hand_value(hand) >= 17
}

#[test]
fn test_dealer_stands() {
    assert!(dealer_stands(&vec![10, 7]));
    assert!(dealer_stands(&vec![10, 6, 6]));
    assert!(dealer_stands(&vec![6, 1]));
    assert!(dealer_stands(&vec![1, 1, 1, 1, 2, 2]));
    assert!(!dealer_stands(&vec![10, 6]));
}

fn next_card_isnt_ten(hand: &Vec<u16>, failed_insurance: bool) -> bool {
    hand.len() == 1 && failed_insurance
}

fn dealer_scores(mut deck: &mut Deck,
                 mut hand: &mut Vec<u16>,
                 failed_insurance: bool)
                 -> HashMap<Score, f32> {
    let mut score_probabilities = HashMap::new();
    if dealer_stands(&hand) {
        score_probabilities.insert(score(&hand), 1.0);
        return score_probabilities;
    }
    let cant_be_ten = next_card_isnt_ten(&hand, failed_insurance);
    let max_card = if cant_be_ten { 9 } else { 10 };
    for card in 1..(max_card + 1) {
        let draw_prob = deck.card_prob(card, cant_be_ten);
        if draw_prob == 0.0 {
            continue;
        }
        deck.draw_to(&mut hand, card);
        let draw_scores = dealer_scores(&mut deck, &mut hand, failed_insurance);
        deck.replace_from(&mut hand, card);
        for (score, prob) in draw_scores.iter() {
            let current_prob = score_probabilities.entry(*score).or_insert(0.0);
            *current_prob += *prob * draw_prob;
        }
    }
    score_probabilities
}

#[test]
fn test_dealer_scores() {
    let mut result_map = HashMap::new();
    result_map.insert(Score::Natural, 1.0);
    assert!(result_map == dealer_scores(&mut FULL_DECK.clone(), &mut vec![1, 10], false));

    result_map = HashMap::new();
    result_map.insert(Score::SixCardCharlie(18), 1.0);
    assert!(result_map == dealer_scores(&mut FULL_DECK.clone(), &mut vec![1, 1, 1, 1, 2, 2], false));

    result_map = HashMap::new();
    result_map.insert(Score::Value(17), 1.0);
    assert!(result_map == dealer_scores(&mut FULL_DECK.clone(), &mut vec![10, 7], false));

    result_map = HashMap::new();
    result_map.insert(Score::Bust, 1.0);
    assert!(result_map ==
            dealer_scores(&mut Deck {
                              cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
                              size: 10,
                          },
                          &mut vec![10, 6],
                          false));

    result_map = HashMap::new();
    result_map.insert(Score::Bust, 0.5);
    result_map.insert(Score::Value(20), 0.5);
    assert!(result_map ==
            dealer_scores(&mut Deck {
                              cards: [0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
                              size: 2,
                          },
                          &mut vec![10, 5],
                          false));

    result_map = HashMap::new();
    result_map.insert(Score::Value(19), 1.0);
    assert!(result_map ==
            dealer_scores(&mut Deck {
                              cards: [0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
                              size: 2,
                          },
                          &mut vec![10],
                          true));

    result_map = HashMap::new();
    result_map.insert(Score::Value(19), 1.0);
    assert!(result_map ==
            dealer_scores(&mut Deck {
                              cards: [0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
                              size: 1,
                          },
                          &mut vec![10],
                          true));
}

fn stand_expectation(state: &mut GameState) -> f32 {
    let player_score = score(&state.player);
    if player_score == Score::Bust {
        return -1.0;
    }
    let score_probabilities =
        dealer_scores(&mut state.deck, &mut state.dealer, state.failed_insurance);

    score_probabilities.iter()
        .map(|(&dealer_score, prob)| hand_expectation(player_score, dealer_score) * prob)
        .sum()
}

#[test]
fn test_stand_expectation() {
    let mut state = GameState {
        player: vec![1, 10],
        dealer: vec![10, 10],
        deck: FULL_DECK.clone(),
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(1.5 == stand_expectation(&mut state));

    state = GameState {
        player: vec![10, 10],
        dealer: vec![10, 10],
        deck: FULL_DECK.clone(),
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(0.0 == stand_expectation(&mut state));

    state = GameState {
        player: vec![10, 10, 10],
        dealer: vec![10, 10],
        deck: FULL_DECK.clone(),
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(-1.0 == stand_expectation(&mut state));

    state = GameState {
        player: vec![1, 10],
        dealer: vec![10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            size: 1,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(1.5 == stand_expectation(&mut state));

    state = GameState {
        player: vec![10, 10],
        dealer: vec![10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            size: 2,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(0.5 == stand_expectation(&mut state));

    state = GameState {
        player: vec![10, 10],
        dealer: vec![10, 5],
        deck: Deck {
            cards: [0, 0, 0, 0, 3, 0, 0, 0, 0, 1],
            size: 4,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(0.25 == stand_expectation(&mut state));

    state = GameState {
        player: vec![10, 10],
        dealer: vec![1],
        deck: Deck {
            cards: [4, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            size: 5,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    println!("Stand expectation: {}", stand_expectation(&mut state));
    assert!(-1.0 == stand_expectation(&mut state));

    state = GameState {
        player: vec![10, 10],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 1, 0, 1],
            size: 2,
        },
        failed_insurance: true,
        is_split: false,
        first_split_hand: false,
    };
    assert!(1.0 == stand_expectation(&mut state));
}

fn double_expectation(mut state: &mut GameState) -> f32 {
    let mut total_expectation = 0.0;
    for card in 1..11 {
        let draw_prob = state.deck.card_prob(card, false);
        if draw_prob == 0.0 {
            continue;
        }
        state.deck.draw_to(&mut state.player, card);
        total_expectation += draw_prob * stand_expectation(&mut state);
        state.deck.replace_from(&mut state.player, card);
    }
    2.0 * total_expectation
}

#[test]
fn test_double_expectation() {
    let mut state = GameState {
        player: vec![10, 10],
        dealer: vec![10, 10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            size: 1,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(-2.0 == double_expectation(&mut state));

    state = GameState {
        player: vec![5, 5],
        dealer: vec![10, 9],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            size: 1,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(2.0 == double_expectation(&mut state));

    state = GameState {
        player: vec![5, 5],
        dealer: vec![10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            size: 2,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(0.0 == double_expectation(&mut state));
}

// #[test]
// #[should_panic]
// fn test_cant_double_on_first_split_hand() {
//     let mut state = GameState {
//         player: vec![10, 10],
//         dealer: vec![10, 10],
//         deck: Deck {
//             cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
//             size: 1,
//         },
//         failed_insurance: false,
//         is_split: true,
//         first_split_hand: true,
//     };
//     double_expectation(&mut state);
// }

fn hit_expectation(mut state: &mut GameState) -> f32 {
    assert!(Score::Bust != score(&state.player));
    let mut total_expectation = 0.0;
    for card in 1..11 {
        let draw_prob = state.deck.card_prob(card, false);
        if draw_prob == 0.0 {
            continue;
        }
        state.deck.draw_to(&mut state.player, card);
        total_expectation += draw_prob * expectation(&mut state);
        state.deck.replace_from(&mut state.player, card);
    }
    total_expectation
}

#[test]
fn test_hit_expectation() {
    let mut state = GameState {
        player: vec![10, 10],
        dealer: vec![10, 10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            size: 1,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(-1.0 == hit_expectation(&mut state));

    state = GameState {
        player: vec![5, 5],
        dealer: vec![10, 9],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(1.0 == hit_expectation(&mut state));

    state = GameState {
        player: vec![5, 5],
        dealer: vec![10, 7],
        deck: Deck {
            cards: [0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
            size: 3,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(1.0 == hit_expectation(&mut state));

    state = GameState {
        player: vec![1],
        dealer: vec![10],
        deck: Deck {
            cards: [2, 4, 4, 4, 4, 4, 4, 4, 4, 15],
            size: 49,
        },
        failed_insurance: false,
        is_split: true,
        first_split_hand: true,
    };
    assert!(3.5 > hit_expectation(&mut state));

}

#[test]
#[should_panic]
fn test_invalid_hit_expectation() {
    let mut state = GameState {
        player: vec![10, 10, 10],
        dealer: vec![10, 10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            size: 1,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    hit_expectation(&mut state);
}

fn insurance_expectation(mut state: &mut GameState) -> f32 {
    assert!(state.dealer == vec![1] && state.player.len() == 2 && !state.is_split);
    let mut total_expectation = 0.0;
    if score(&state.player) == Score::Natural {
        return 1.0;
    }
    if state.deck.size == state.deck.cards[9] {
        return 0.0;
    }
    state.failed_insurance = true;
    total_expectation += (1.0 - state.deck.card_prob(10, false)) * (expectation(state) - 0.5);
    state.failed_insurance = false;
    total_expectation
}

#[test]
fn test_insurance_expectation() {
    let mut state = GameState {
        player: vec![10, 10],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(0.0 == insurance_expectation(&mut state));

    state = GameState {
        player: vec![1, 10],
        dealer: vec![1],
        deck: FULL_DECK.clone(),
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(1.0 == insurance_expectation(&mut state));

    state = GameState {
        player: vec![10, 6],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 4, 0],
            size: 4,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(-1.5 == insurance_expectation(&mut state));

    state = GameState {
        player: vec![5, 5],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 2, 0, 2],
            size: 4,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    assert!(-0.25 == insurance_expectation(&mut state));
}

#[test]
#[should_panic]
fn test_non_ace_insurance_expectation() {
    let mut state = GameState {
        player: vec![10, 10],
        dealer: vec![10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            size: 1,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    insurance_expectation(&mut state);
}

#[test]
#[should_panic]
fn test_non_starting_insurance_expectation() {
    let mut state = GameState {
        player: vec![10, 3, 2],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    insurance_expectation(&mut state);
}

#[test]
#[should_panic]
fn test_first_split_hand_insurance_expectation() {
    let mut state = GameState {
        player: vec![10, 3],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: true,
        first_split_hand: true,
    };
    insurance_expectation(&mut state);
}

#[test]
#[should_panic]
fn test_is_split_insurance_expectation() {
    let mut state = GameState {
        player: vec![10, 3],
        dealer: vec![1, 10],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: true,
        first_split_hand: false,
    };
    insurance_expectation(&mut state);
}

fn will_reshuffle(deck: &Deck) -> bool {
    false
}

fn reshuffle_deck(mut state: &mut GameState) {
    state.deck = FULL_DECK.clone();
    state.deck.draw(state.player[0]);
    state.deck.draw(state.player[0]);
    state.deck.draw(state.dealer[0]);
    if state.dealer.len() == 2 {
        state.deck.draw(state.dealer[1]);
    }
}

#[test]
fn test_reshuffle_deck() {
    let mut state = GameState {
        player: vec![10, 3],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    reshuffle_deck(&mut state);
    assert!(49 == state.deck.size);
    assert!(3 == state.deck.cards[0]);

    state = GameState {
        player: vec![10, 3],
        dealer: vec![1, 4],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    reshuffle_deck(&mut state);
    assert!(48 == state.deck.size);
    assert!(3 == state.deck.cards[0]);
    assert!(3 == state.deck.cards[3])
}

fn split_expectation(mut state: &mut GameState) -> f32 {
    assert!(state.player.len() == 2 && state.player[0] == state.player[1] && !state.is_split &&
            !state.failed_insurance);
    let mut total_expectation = 0.0;
    state.is_split = true;
    state.player.pop();
    let original_deck = state.deck.clone();
    state.deck = FULL_DECK.clone();
    total_expectation += 2.0 * hit_expectation(&mut state);
    // println!("Expectation after first hand: {}", total_expectation);
    state.is_split = false;
    let player_card = state.player[0];
    state.player.push(player_card);
    state.deck = original_deck;
    total_expectation
}

// #[test]
// fn test_split_expectation() {
//     let mut state = GameState {
//         player: vec![1, 1],
//         dealer: vec![7],
//         deck: Deck {
//             cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 30],
//             size: 30,
//         },
//         failed_insurance: false,
//         is_split: false,
//         first_split_hand: false,
//     };
//     assert!(3.5 == split_expectation(&mut state));

//     state = GameState {
//         player: vec![10, 10],
//         dealer: vec![7],
//         deck: Deck {
//             cards: [0, 0, 0, 0, 0, 30, 0, 0, 0, 0],
//             size: 30,
//         },
//         failed_insurance: false,
//         is_split: false,
//         first_split_hand: false,
//     };
//     assert!(-1.5 == split_expectation(&mut state));

//     state = GameState {
//         player: vec![1, 1],
//         dealer: vec![1],
//         deck: Deck {
//             cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 30],
//             size: 30,
//         },
//         failed_insurance: false,
//         is_split: false,
//         first_split_hand: false,
//     };
//     assert!(0.0 == split_expectation(&mut state));

//     state = GameState {
//         player: vec![1, 1],
//         dealer: vec![10],
//         deck: Deck {
//             cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
//             size: 10,
//         },
//         failed_insurance: false,
//         is_split: false,
//         first_split_hand: false,
//     };
//     assert!(3.0 > split_expectation(&mut state));
// }

#[test]
#[should_panic]
fn test_not_two_cards_split_expectation() {
    let mut state = GameState {
        player: vec![3, 3, 2],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    split_expectation(&mut state);
}

#[test]
#[should_panic]
fn test_not_same_cards_split_expectation() {
    let mut state = GameState {
        player: vec![2, 3],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    split_expectation(&mut state);
}

#[test]
#[should_panic]
fn test_not_is_split_split_expectation() {
    let mut state = GameState {
        player: vec![2, 2],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: false,
        is_split: true,
        first_split_hand: false,
    };
    split_expectation(&mut state);
}

#[test]
#[should_panic]
fn test_not_failed_insurance_split_expectation() {
    let mut state = GameState {
        player: vec![2, 2],
        dealer: vec![1],
        deck: Deck {
            cards: [0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
            size: 10,
        },
        failed_insurance: true,
        is_split: false,
        first_split_hand: false,
    };
    split_expectation(&mut state);
}

fn cannot_hit(player: &Vec<u16>) -> bool {
    player.len() == 6 || min_hand_value(player) >= 21
}

fn can_insurance(state: &GameState) -> bool {
    state.dealer.len() == 1 && state.dealer[0] == 1 && state.player.len() == 2 &&
    !state.failed_insurance && !state.is_split
}

fn can_surrender(state: &GameState) -> bool {
    state.player.len() == 2 && !state.first_split_hand && !state.failed_insurance
}

fn can_double(state: &GameState) -> bool {
    state.player.len() == 2 && !state.failed_insurance
}

fn can_split(state: &GameState) -> bool {
    state.player.len() == 2 && state.player[0] == state.player[1] && !state.is_split &&
    !state.failed_insurance
}

fn max(m: f32, n: f32) -> f32 {
    if m > n { m } else { n }
}

fn expectation(mut state: &mut GameState) -> f32 {
    let mut max_expectation = stand_expectation(&mut state);
    if cannot_hit(&state.player) {
        return max_expectation;
    }
    if can_insurance(state) {
        max_expectation = max(max_expectation, insurance_expectation(&mut state));
    }
    if can_surrender(state) {
        max_expectation = max(max_expectation, -0.5);
    }
    if can_double(state) {
        max_expectation = max(max_expectation, double_expectation(&mut state));
    }
    if can_split(state) {
        max_expectation = max(max_expectation, split_expectation(&mut state));
    }
    let best_expectation = max(max_expectation, hit_expectation(&mut state));
    // println!("Game state: {:?}", &state);
    // println!("Expectation: {}", best_expectation);
    best_expectation
}

fn player_hand_expectation(mut state: &mut GameState) -> f32 {
    let mut total_expectation = 0.0;
    for card in 1..11 {
        let draw_prob = state.deck.card_prob(card, false);
        if draw_prob == 0.0 {
            continue;
        }
        state.deck.draw_to(&mut state.dealer, card);
        total_expectation += draw_prob * expectation(&mut state);
        state.deck.replace_from(&mut state.dealer, card);
    }
    total_expectation
}

fn deck_expectation(deck: Deck) -> f32 {
    let mut total_expectation = 0.0;
    let mut state = GameState {
        player: vec![],
        dealer: vec![],
        deck: deck,
        failed_insurance: false,
        is_split: false,
        first_split_hand: false,
    };
    for card1 in 1..11 {
        let draw_prob1 = state.deck.card_prob(card1, false);
        if draw_prob1 == 0.0 {
            continue;
        }
        state.deck.draw_to(&mut state.player, card1);
        for card2 in card1..11 {
            let draw_prob2 = state.deck.card_prob(card2, false);
            if draw_prob2 == 0.0 {
                continue;
            }
            state.deck.draw_to(&mut state.player, card2);
            let hand_expectation = player_hand_expectation(&mut state);
            println!("Expectation for {},{}: {}", card1, card2, hand_expectation);
            if card1 == card2 {
                total_expectation += draw_prob1 * draw_prob2 * hand_expectation;
            } else {
                total_expectation += 2.0 * draw_prob1 * draw_prob2 * hand_expectation;
            }
            state.deck.replace_from(&mut state.player, card2);
        }
        state.deck.replace_from(&mut state.player, card1);
    }
    total_expectation
}

fn all_deck_expectations() -> () {
    let mut deck = FULL_DECK.clone();
    println!("Full deck expectation: {}", deck_expectation(deck));
    for card in 1..11 {
        deck.draw(card);
        println!("Expectation without {}: {}", card, deck_expectation(deck));
        deck.replace(card);
    }
}

// Want to make getting samples cleaner, but not worth the time atm
// fn stringrecord_to_deck(record: &csv::StringRecord) -> Deck {
//     let card_iter = record.iter().take(10);
//     return Deck {
//         cards: card_iter.collect()::Vec<u16>.to_slice(),
//         size: card_iter.sum(),
//     };
// }

fn deck_samples(filename: &str) -> Result<Vec<Deck>, Box<Error>> {
    let mut reader = csv::Reader::from_path(filename)?;
    let mut decks = vec![];
    for record in reader.deserialize() {
        // ew
        let (aces, twos, threes, fours, fives, sixes, sevens, eights, nines, tens): (u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16,
                                                                                     u16) = record?;
        let cards = [aces, twos, threes, fours, fives, sixes, sevens, eights, nines, tens];
        decks.push(Deck {
            cards: cards,
            size: cards.iter().sum(),
        });
    }
    return Ok(decks);
}

fn computed_decks(filename: &str) -> Result<HashSet<Deck>, Box<Error>> {
    let mut reader = csv::Reader::from_path(filename)?;
    let mut decks = HashSet::new();
    for record in reader.deserialize() {
        // ew
        let (aces, twos, threes, fours, fives, sixes, sevens, eights, nines, tens, _): (u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        u16,
                                                                                        f32) =
            record?;
        let cards = [aces, twos, threes, fours, fives, sixes, sevens, eights, nines, tens];
        decks.insert(Deck {
            cards: cards,
            size: cards.iter().sum(),
        });
    }
    return Ok(decks);
}

fn random_deck(samples_path: &str, data_path: &str) -> Result<Deck, Box<Error>> {
    let decks = deck_samples(samples_path)?;
    let computed_decks = computed_decks(data_path)?;
    // println!("Computed decks: {:?}", computed_decks);
    let mut rng = rand::thread_rng();
    let indices = Range::new(0, decks.len());
    loop {
        let index = indices.ind_sample(&mut rng);
        if computed_decks.contains(&decks[index]) {
            continue;
        }
        return Ok(decks[index]);
    }
}

fn append_advantage_data(data_path: &str, deck: Deck, advantage: f32) -> Result<(), Box<Error>> {
    let mut writer =
        csv::Writer::from_writer(OpenOptions::new().append(true).open(data_path).unwrap());
    let mut record =
        csv::StringRecord::from_iter(deck.cards.iter().map(|card: &u16| card.to_string()));
    record.push_field(&advantage.to_string());
    writer.write_record(record.iter())?;
    Ok(())
}

fn continuously_compute_deck_advantages(samples_path: &str, data_path: &str) -> () {
    loop {
        let deck = random_deck(samples_path, data_path).unwrap();
        println!("Computing the advantage of {:?}", deck);
        let start = PreciseTime::now();
        let advantage = deck_expectation(deck);
        let end = PreciseTime::now();
        println!("The advantage of {:?} is {}, ({} seconds)",
                 deck,
                 advantage,
                 start.to(end).num_seconds());
        append_advantage_data(data_path, deck, advantage).unwrap();
    }
}


#[derive(Eq,PartialEq,Hash,Debug,Clone)]
struct OrderedDeck {
    cards: Vec<u16>,
    deck: Deck,
}

impl OrderedDeck {
    fn draw(&mut self) -> u16 {
        let card = self.cards.pop().unwrap();
        self.deck.draw(card);
        card
    }
}

fn parse_deck(deck_str: &str) -> Deck {
    let mut cards: Vec<u16> = deck_str.chars().map(|letter| letter.to_digit(10).unwrap() as u16).collect();
    if (cards.len() == 11) {
        cards[9] = 10 + cards[10];
        cards.pop();
    }
    let mut array = [0u16; 10];
    for (&x, p) in cards.iter().zip(array.iter_mut()) {
        *p = x;
    }
    Deck {
        cards: array,
        size: cards.iter().sum()
    }
}

fn parse_hand(hand_str: &str) -> Vec<u16> {
    hand_str.chars()
        .map(|letter| letter.to_digit(10).unwrap())
        .map(|num| {
            if num == 0 {
                10 as u16
            } else {
                num as u16
            }
        })
        .collect()
}

fn best_action(mut state: &mut GameState) -> String {
    let mut best_expectation = stand_expectation(&mut state);
    let mut best_action = "Stand";
    let hit_exp = hit_expectation(&mut state);
    if hit_exp > best_expectation {
        best_expectation = hit_exp;
        best_action = "Hit";
    }
    if can_double(state) {
        let double_exp = double_expectation(&mut state);
        if double_exp > best_expectation {
            best_expectation = double_exp;
            best_action = "Double";
        }
    }
    if can_split(state) {
        let split_exp = split_expectation(&mut state);
        if split_exp > best_expectation {
            best_expectation = split_exp;
            best_action = "Split";
        }
    }
    String::from(best_action)
}

fn main() {
    let exp = deck_expectation(FULL_DECK.clone());
    println!("Deck expectation: {}", exp);
    // PROFILER.lock().unwrap().start("./baseline-1,1v10.profile").unwrap();
    // let mut state = GameState {
    //     player: vec![10, 2],
    //     dealer: vec![6],
    //     deck: Deck {
    //         cards: [4, 3, 4, 4, 4, 3, 4, 4, 4, 15],
    //         size: 49,
    //     },
    //     failed_insurance: false,
    //     is_split: false,
    //     first_split_hand: false,
    // };
    // println!("Stand Expectation: {}", stand_expectation(&mut state));
    // println!("Double Expectation: {}", double_expectation(&mut state));
    // println!("Hit Expectation: {}", hit_expectation(&mut state));
    // println!("Best action: {}", best_action(&mut state));
    // all_deck_expectations();
    // println!("Number of decks: {}",
    //          deck_samples("/home/chris/coding/advantage_calculator/large_decks.csv")
    //              .unwrap()
    //              .len());
    // println!("Number of decks: {}",
    //          computed_decks("/home/chris/coding/advantage_calculator/data.csv").unwrap().len());
    // println!("Random deck: {:?}",
    //          random_deck("/home/chris/coding/advantage_calculator/large_decks.csv",
    //                      "/home/chris/coding/advantage_calculator/data.csv")
    //              .unwrap());
    // println!("Path: {:?}", Path::new("./decks.csv"));
    // println!("The current directory is {}",
    //          env::current_dir().unwrap().display());
    // let args: Vec<String> = env::args().collect();
    // let num_threads = args[1].parse::<i32>().unwrap();
    // let mut handles = vec![];
    // for _ in 1..(num_threads + 1) {
    //     handles.push(thread::spawn(move || {
    //         continuously_compute_deck_advantages("./decks.csv", "./data.csv");
    //     }));
    // }
    // for handle in handles {
    //     let _ = handle.join();
    // }

    // continuously_compute_deck_advantages("/home/chris/coding/advantage_calculator/large_decks.csv",
    //                                      "/home/chris/coding/blackjack_sim/data.csv");
    // println!("{:?}",
    //          computed_decks("/home/chris/coding/blackjack_sim/data.csv")
    //              .unwrap()
    //              .contains(&FULL_DECK));

    // let decks = deck_set("/home/chris/coding/advantage_calculator/large_decks.csv");
    // for _ in 1..100 {
    //     println!("Random deck: {:?}",
    //              random_deck("/home/chris/coding/advantage_calculator/large_decks.csv",
    //                          "/home/chris/coding/blackjack_sim/data.csv"));
    // }
    // Code you want to sample goes here!
    // PROFILER.lock().unwrap().stop().unwrap();
}
