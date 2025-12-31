import 'package:json_annotation/json_annotation.dart';

part 'memo_order_entity.g.dart';

@JsonSerializable()
class MemoOrderEntity {
  static const tableName = "memo_order";

  MemoOrderEntity({this.displaymode, required this.id, required this.seq});

  int? displaymode;
  String id;
  int seq;

  factory MemoOrderEntity.fromJson(Map<String, dynamic> json) =>
      _$MemoOrderEntityFromJson(json);

  Map<String, dynamic> toJson() => _$MemoOrderEntityToJson(this);
}
