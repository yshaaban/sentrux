extends Node2D

class_name Player

signal health_changed(new_health)

var speed = 200.0
@export var max_health: int = 100

func _ready():
	var scene = preload("res://scenes/bullet.tscn")
	var config = load("res://config.gd")
	print("Player ready")

func _process(delta):
	var velocity = Vector2.ZERO
	if Input.is_action_pressed("move_right"):
		velocity.x += 1
	position += velocity * speed * delta

func take_damage(amount: int) -> void:
	max_health -= amount
	health_changed.emit(max_health)
	if max_health <= 0:
		die()

func die():
	queue_free()

class Inventory:
	var items = []

	func add_item(item):
		items.append(item)

	func remove_item(item):
		items.erase(item)
